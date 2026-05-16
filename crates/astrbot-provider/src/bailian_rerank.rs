use std::collections::HashMap;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{RerankDocumentScore, RerankProvider, RerankRequest, RerankResponse};

const ERROR_TEXT_MAX_CHARS: usize = 4096;
const MAX_DOCUMENTS: usize = 500;

#[derive(Clone, Debug)]
pub struct BailianRerankConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub return_documents: bool,
    pub instruct: Option<String>,
}

impl BailianRerankConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            model: model.into(),
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            return_documents: false,
            instruct: None,
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.insert(key.into(), value.into());
        self
    }

    pub fn with_return_documents(mut self, return_documents: bool) -> Self {
        self.return_documents = return_documents;
        self
    }

    pub fn with_instruct(mut self, instruct: impl Into<String>) -> Self {
        self.instruct = Some(instruct.into());
        self
    }

    fn rerank_url(&self) -> String {
        self.api_base.trim_end_matches('/').to_string()
    }
}

#[derive(Clone, Debug)]
pub struct BailianRerankProvider {
    config: BailianRerankConfig,
    client: reqwest::Client,
}

impl BailianRerankProvider {
    pub fn new(config: BailianRerankConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .default_headers(build_headers(&config)?)
            .build()
            .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))?;

        Ok(Self { config, client })
    }

    fn build_payload(&self, request: &RerankRequest) -> Result<BailianRerankRequest> {
        if request.documents.is_empty() {
            return Err(AstrbotError::Provider(
                "rerank request must contain at least one document".to_string(),
            ));
        }

        let model = request
            .model
            .clone()
            .unwrap_or_else(|| self.config.model.clone());
        let documents = request
            .documents
            .iter()
            .take(MAX_DOCUMENTS)
            .cloned()
            .collect::<Vec<_>>();
        let parameters = self.build_parameters(&model, request.top_n);

        Ok(BailianRerankRequest {
            model,
            input: BailianRerankInput {
                query: request.query.clone(),
                documents,
            },
            parameters,
        })
    }

    fn build_parameters(
        &self,
        model: &str,
        top_n: Option<usize>,
    ) -> Option<BailianRerankParameters> {
        let top_n = top_n.filter(|top_n| *top_n > 0);
        let return_documents = self.config.return_documents.then_some(true);
        let instruct = self
            .config
            .instruct
            .as_ref()
            .filter(|instruct| !instruct.trim().is_empty() && model == "qwen3-rerank")
            .cloned();

        if top_n.is_none() && return_documents.is_none() && instruct.is_none() {
            return None;
        }

        Some(BailianRerankParameters {
            top_n,
            return_documents,
            instruct,
        })
    }
}

#[async_trait]
impl RerankProvider for BailianRerankProvider {
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse> {
        let response = self
            .client
            .post(self.config.rerank_url())
            .json(&self.build_payload(&request)?)
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Bailian rerank request failed: {err}"))
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "Bailian rerank provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        let payload: BailianRerankResponse = serde_json::from_str(&body).map_err(|err| {
            AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
        })?;

        if payload.code.as_deref().is_some_and(|code| code != "200") {
            return Err(AstrbotError::Provider(format!(
                "Bailian rerank provider returned code {}: {}",
                payload.code.unwrap_or_default(),
                payload.message.unwrap_or_default()
            )));
        }

        let results = payload
            .output
            .map(|output| output.results)
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(position, result)| {
                RerankDocumentScore::new(
                    result.index.unwrap_or(position),
                    result.relevance_score.unwrap_or(0.0),
                )
            })
            .collect();

        Ok(RerankResponse::new(results))
    }
}

#[derive(Debug, Serialize)]
struct BailianRerankRequest {
    model: String,
    input: BailianRerankInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<BailianRerankParameters>,
}

#[derive(Debug, Serialize)]
struct BailianRerankInput {
    query: String,
    documents: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BailianRerankParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    top_n: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_documents: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instruct: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BailianRerankResponse {
    code: Option<String>,
    message: Option<String>,
    output: Option<BailianRerankOutput>,
}

#[derive(Debug, Deserialize)]
struct BailianRerankOutput {
    #[serde(default)]
    results: Vec<BailianRerankResult>,
}

#[derive(Debug, Deserialize)]
struct BailianRerankResult {
    index: Option<usize>,
    relevance_score: Option<f32>,
}

fn build_headers(config: &BailianRerankConfig) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let api_key = config
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|api_key| !api_key.is_empty())
        .ok_or_else(|| AstrbotError::Provider("Bailian rerank API key is required".to_string()))?;
    let bearer = format!("Bearer {api_key}");
    let value = HeaderValue::from_str(&bearer)
        .map_err(|_| AstrbotError::Provider("invalid Bailian rerank API key header".to_string()))?;
    headers.insert(AUTHORIZATION, value);

    for (key, value) in &config.custom_headers {
        let name = HeaderName::from_bytes(key.as_bytes()).map_err(|_| {
            AstrbotError::Provider(format!("invalid custom provider header name: {key}"))
        })?;
        let value = HeaderValue::from_str(value).map_err(|_| {
            AstrbotError::Provider(format!("invalid custom provider header value for: {key}"))
        })?;
        headers.insert(name, value);
    }

    Ok(headers)
}

fn extract_error_message(body: &str) -> String {
    let fallback = truncate(body.trim());
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return fallback;
    };

    let extracted = value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .or_else(|| value.get("message").and_then(Value::as_str))
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_string);

    extracted.unwrap_or(fallback)
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= ERROR_TEXT_MAX_CHARS {
        return text.to_string();
    }

    text.chars().take(ERROR_TEXT_MAX_CHARS).collect()
}
