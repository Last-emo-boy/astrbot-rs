use std::collections::HashMap;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::http::{build_http_client, extract_error_message, join_api_path, json_bearer_headers};
use crate::{EmbeddingProvider, EmbeddingRequest, EmbeddingResponse};

#[derive(Clone, Debug)]
pub struct OpenAiEmbeddingConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub dimensions: Option<usize>,
}

impl OpenAiEmbeddingConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            model: model.into(),
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            dimensions: Some(1024),
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

    fn embeddings_url(&self) -> String {
        join_api_path(&self.api_base, "embeddings")
    }
}

#[derive(Clone, Debug)]
pub struct OpenAiEmbeddingProvider {
    config: OpenAiEmbeddingConfig,
    client: reqwest::Client,
}

impl OpenAiEmbeddingProvider {
    pub fn new(config: OpenAiEmbeddingConfig) -> Result<Self> {
        let client = build_http_client(
            config.timeout,
            json_bearer_headers(
                config.api_key.as_deref(),
                &config.custom_headers,
                "invalid OpenAI embedding API key header",
            )?,
        )?;

        Ok(Self { config, client })
    }

    fn build_payload(&self, request: &EmbeddingRequest) -> Result<OpenAiEmbeddingRequest> {
        if request.texts.is_empty() {
            return Err(AstrbotError::Provider(
                "embedding request must contain at least one text".to_string(),
            ));
        }

        let input = if let [text] = request.texts.as_slice() {
            OpenAiEmbeddingInput::Text(text.clone())
        } else {
            OpenAiEmbeddingInput::Batch(request.texts.clone())
        };

        Ok(OpenAiEmbeddingRequest {
            model: request
                .model
                .clone()
                .unwrap_or_else(|| self.config.model.clone()),
            input,
            dimensions: self.config.dimensions,
        })
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAiEmbeddingProvider {
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse> {
        let response = self
            .client
            .post(self.config.embeddings_url())
            .json(&self.build_payload(&request)?)
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("OpenAI embedding request failed: {err}"))
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "OpenAI embedding provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        let mut payload: OpenAiEmbeddingResponse = serde_json::from_str(&body).map_err(|err| {
            AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
        })?;
        payload.data.sort_by_key(|item| item.index);
        let embeddings = payload
            .data
            .into_iter()
            .map(|item| item.embedding)
            .collect::<Vec<_>>();

        if embeddings.is_empty() {
            return Err(AstrbotError::Provider(
                "provider response did not contain embeddings".to_string(),
            ));
        }

        Ok(EmbeddingResponse::new(embeddings))
    }

    fn dimensions(&self) -> Option<usize> {
        self.config.dimensions
    }
}

#[derive(Debug, Serialize)]
struct OpenAiEmbeddingRequest {
    model: String,
    input: OpenAiEmbeddingInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OpenAiEmbeddingInput {
    Text(String),
    Batch(Vec<String>),
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingData {
    embedding: Vec<f32>,
    #[serde(default)]
    index: usize,
}
