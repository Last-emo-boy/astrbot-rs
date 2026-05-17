use std::collections::HashMap;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::HeaderName;

use crate::http::{build_http_client, extract_error_message, json_api_key_headers};
use crate::protocol::gemini_chat::{
    GeminiGenerateContentRequest, build_gemini_generate_content_request, extract_gemini_response,
};
use crate::streaming::reject_unsupported_streaming;
use crate::{ChatProvider, ChatRequest, ChatResponse};

#[derive(Clone, Debug)]
pub struct GeminiConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
}

impl GeminiConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            model: model.into(),
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
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

    fn generate_content_url(&self, model: &str) -> String {
        let api_base = self.api_base.trim_end_matches('/');
        let model = model.trim_start_matches("models/");
        if api_base.ends_with("/v1beta") {
            format!("{api_base}/models/{model}:generateContent")
        } else {
            format!("{api_base}/v1beta/models/{model}:generateContent")
        }
    }
}

#[derive(Clone, Debug)]
pub struct GeminiProvider {
    config: GeminiConfig,
    client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(config: GeminiConfig) -> Result<Self> {
        let client = build_http_client(config.timeout, build_headers(&config)?)?;

        Ok(Self { config, client })
    }

    fn build_payload(&self, request: &ChatRequest) -> Result<GeminiGenerateContentRequest> {
        build_gemini_generate_content_request(request)
    }
}

#[async_trait]
impl ChatProvider for GeminiProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        reject_unsupported_streaming("Gemini", request.stream)?;

        let model = request
            .model
            .clone()
            .unwrap_or_else(|| self.config.model.clone());
        let response = self
            .client
            .post(self.config.generate_content_url(&model))
            .json(&self.build_payload(&request)?)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("Gemini request failed: {err}")))?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "Gemini provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        let response = extract_gemini_response(&body)?;
        if response.chain.plain_text().trim().is_empty() {
            return Err(AstrbotError::Provider(
                "provider response did not contain assistant content".to_string(),
            ));
        }

        Ok(ChatResponse {
            chain: response.chain,
            metadata: response.metadata,
        })
    }
}

fn build_headers(config: &GeminiConfig) -> Result<reqwest::header::HeaderMap> {
    json_api_key_headers(
        HeaderName::from_static("x-goog-api-key"),
        config.api_key.as_deref(),
        &config.custom_headers,
        "invalid Gemini API key header",
    )
}
