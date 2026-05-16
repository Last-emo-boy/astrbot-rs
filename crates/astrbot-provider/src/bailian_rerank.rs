use std::collections::HashMap;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

use crate::protocol::rerank::{
    build_bailian_rerank_request, extract_bailian_rerank_error_message,
    parse_bailian_rerank_response,
};
use crate::{RerankProvider, RerankRequest, RerankResponse};

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

    fn build_payload(&self, request: &RerankRequest) -> Result<impl serde::Serialize + use<>> {
        build_bailian_rerank_request(
            request,
            &self.config.model,
            self.config.return_documents,
            self.config.instruct.as_deref(),
        )
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
                extract_bailian_rerank_error_message(&body)
            )));
        }

        Ok(RerankResponse::new(parse_bailian_rerank_response(&body)?))
    }
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
