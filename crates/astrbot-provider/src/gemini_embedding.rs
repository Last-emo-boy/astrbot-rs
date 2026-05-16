use std::collections::HashMap;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

use crate::protocol::gemini_embedding::{
    build_gemini_batch_embed_contents_request, build_gemini_embed_content_request,
    extract_gemini_embedding_error_message, gemini_embedding_method_url,
    parse_gemini_batch_embed_contents_response, parse_gemini_embed_content_response,
};
use crate::{EmbeddingProvider, EmbeddingRequest, EmbeddingResponse};

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

    fn embed_content_url(&self, model: &str) -> String {
        gemini_embedding_method_url(&self.api_base, model, "embedContent")
    }

    fn batch_embed_contents_url(&self, model: &str) -> String {
        gemini_embedding_method_url(&self.api_base, model, "batchEmbedContents")
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
            .json(&build_gemini_embed_content_request(
                model,
                text,
                self.config.dimensions,
            ))
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Gemini embedding request failed: {err}"))
            })?;

        let body = response_body_or_error(response, "Gemini embedding provider").await?;
        Ok(EmbeddingResponse::new(vec![
            parse_gemini_embed_content_response(&body)?,
        ]))
    }

    async fn embed_batch(&self, model: &str, texts: &[String]) -> Result<EmbeddingResponse> {
        let response = self
            .client
            .post(self.config.batch_embed_contents_url(model))
            .json(&build_gemini_batch_embed_contents_request(
                model,
                texts,
                self.config.dimensions,
            ))
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Gemini batch embedding request failed: {err}"))
            })?;

        let body = response_body_or_error(response, "Gemini batch embedding provider").await?;
        Ok(EmbeddingResponse::new(
            parse_gemini_batch_embed_contents_response(&body)?,
        ))
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
            extract_gemini_embedding_error_message(&body)
        )));
    }

    Ok(body)
}
