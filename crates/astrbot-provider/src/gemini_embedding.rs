use std::collections::HashMap;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{EmbeddingProvider, EmbeddingRequest, EmbeddingResponse};

const ERROR_TEXT_MAX_CHARS: usize = 4096;

#[derive(Clone, Debug)]
pub struct GeminiEmbeddingConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub dimensions: Option<usize>,
}

impl GeminiEmbeddingConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            model: model.into(),
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            dimensions: Some(768),
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

    pub fn with_dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    fn model_resource(&self, model: &str) -> String {
        if model.starts_with("models/") {
            model.to_string()
        } else {
            format!("models/{model}")
        }
    }

    fn embed_content_url(&self, model: &str) -> String {
        self.model_url(model, "embedContent")
    }

    fn batch_embed_contents_url(&self, model: &str) -> String {
        self.model_url(model, "batchEmbedContents")
    }

    fn model_url(&self, model: &str, method: &str) -> String {
        let api_base = self.api_base.trim_end_matches('/');
        let model = self.model_resource(model);
        if api_base.ends_with("/v1beta") {
            format!("{api_base}/{model}:{method}")
        } else {
            format!("{api_base}/v1beta/{model}:{method}")
        }
    }
}

#[derive(Clone, Debug)]
pub struct GeminiEmbeddingProvider {
    config: GeminiEmbeddingConfig,
    client: reqwest::Client,
}

impl GeminiEmbeddingProvider {
    pub fn new(config: GeminiEmbeddingConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .default_headers(build_headers(&config)?)
            .build()
            .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))?;

        Ok(Self { config, client })
    }

    fn model_for_request(&self, request: &EmbeddingRequest) -> String {
        request
            .model
            .clone()
            .unwrap_or_else(|| self.config.model.clone())
    }

    fn build_embed_payload(&self, model: &str, text: &str) -> GeminiEmbedContentRequest {
        GeminiEmbedContentRequest {
            model: self.config.model_resource(model),
            content: gemini_text_content(text),
            output_dimensionality: self.config.dimensions,
        }
    }

    fn build_batch_payload(
        &self,
        model: &str,
        texts: &[String],
    ) -> GeminiBatchEmbedContentsRequest {
        GeminiBatchEmbedContentsRequest {
            requests: texts
                .iter()
                .map(|text| self.build_embed_payload(model, text))
                .collect(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for GeminiEmbeddingProvider {
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse> {
        if request.texts.is_empty() {
            return Err(AstrbotError::Provider(
                "embedding request must contain at least one text".to_string(),
            ));
        }

        let model = self.model_for_request(&request);
        if let [text] = request.texts.as_slice() {
            return self.embed_single(&model, text).await;
        }

        self.embed_batch(&model, &request.texts).await
    }

    fn dimensions(&self) -> Option<usize> {
        self.config.dimensions
    }
}

impl GeminiEmbeddingProvider {
    async fn embed_single(&self, model: &str, text: &str) -> Result<EmbeddingResponse> {
        let response = self
            .client
            .post(self.config.embed_content_url(model))
            .json(&self.build_embed_payload(model, text))
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Gemini embedding request failed: {err}"))
            })?;

        let body = response_body_or_error(response, "Gemini embedding provider").await?;
        let payload: GeminiEmbedContentResponse = serde_json::from_str(&body).map_err(|err| {
            AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
        })?;

        if payload.embedding.values.is_empty() {
            return Err(AstrbotError::Provider(
                "provider response did not contain embedding values".to_string(),
            ));
        }

        Ok(EmbeddingResponse::new(vec![payload.embedding.values]))
    }

    async fn embed_batch(&self, model: &str, texts: &[String]) -> Result<EmbeddingResponse> {
        let response = self
            .client
            .post(self.config.batch_embed_contents_url(model))
            .json(&self.build_batch_payload(model, texts))
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Gemini batch embedding request failed: {err}"))
            })?;

        let body = response_body_or_error(response, "Gemini batch embedding provider").await?;
        let payload: GeminiBatchEmbedContentsResponse =
            serde_json::from_str(&body).map_err(|err| {
                AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
            })?;
        let embeddings = payload
            .embeddings
            .into_iter()
            .map(|embedding| embedding.values)
            .collect::<Vec<_>>();

        if embeddings.is_empty() {
            return Err(AstrbotError::Provider(
                "provider response did not contain embeddings".to_string(),
            ));
        }

        Ok(EmbeddingResponse::new(embeddings))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiEmbedContentRequest {
    model: String,
    content: GeminiContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_dimensionality: Option<usize>,
}

#[derive(Debug, Serialize)]
struct GeminiBatchEmbedContentsRequest {
    requests: Vec<GeminiEmbedContentRequest>,
}

#[derive(Debug, Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GeminiEmbedContentResponse {
    embedding: GeminiContentEmbedding,
}

#[derive(Debug, Deserialize)]
struct GeminiBatchEmbedContentsResponse {
    embeddings: Vec<GeminiContentEmbedding>,
}

#[derive(Debug, Deserialize)]
struct GeminiContentEmbedding {
    values: Vec<f32>,
}

fn gemini_text_content(text: &str) -> GeminiContent {
    GeminiContent {
        parts: vec![GeminiPart {
            text: text.to_string(),
        }],
    }
}

fn build_headers(config: &GeminiEmbeddingConfig) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    if let Some(api_key) = config
        .api_key
        .as_deref()
        .filter(|api_key| !api_key.trim().is_empty())
    {
        let value = HeaderValue::from_str(api_key).map_err(|_| {
            AstrbotError::Provider("invalid Gemini embedding API key header".to_string())
        })?;
        headers.insert("x-goog-api-key", value);
    }

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

async fn response_body_or_error(response: reqwest::Response, label: &str) -> Result<String> {
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        AstrbotError::Provider(format!("failed to read provider response: {err}"))
    })?;

    if !status.is_success() {
        return Err(AstrbotError::Provider(format!(
            "{label} returned {status}: {}",
            extract_error_message(&body)
        )));
    }

    Ok(body)
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
