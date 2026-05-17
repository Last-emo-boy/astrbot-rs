use std::collections::HashMap;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

use crate::http::{build_http_client, extract_error_message, insert_custom_headers};
use crate::protocol::anthropic_chat::{
    AnthropicMessageRequest, build_anthropic_message_request, extract_anthropic_response,
};
use crate::streaming::reject_unsupported_streaming;
use crate::{ChatProvider, ChatRequest, ChatResponse};

const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Clone, Debug)]
pub struct AnthropicConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub max_tokens: u32,
}

impl AnthropicConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            model: model.into(),
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            max_tokens: 1024,
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

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    fn messages_url(&self) -> String {
        let api_base = self.api_base.trim_end_matches('/');
        if api_base.ends_with("/v1") {
            format!("{api_base}/messages")
        } else {
            format!("{api_base}/v1/messages")
        }
    }
}

#[derive(Clone, Debug)]
pub struct AnthropicProvider {
    config: AnthropicConfig,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(config: AnthropicConfig) -> Result<Self> {
        let client = build_http_client(config.timeout, build_headers(&config)?)?;

        Ok(Self { config, client })
    }

    fn build_payload(&self, request: &ChatRequest) -> Result<AnthropicMessageRequest> {
        build_anthropic_message_request(request, &self.config.model, self.config.max_tokens)
    }
}

#[async_trait]
impl ChatProvider for AnthropicProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        reject_unsupported_streaming("Anthropic", request.stream)?;

        let response = self
            .client
            .post(self.config.messages_url())
            .json(&self.build_payload(&request)?)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("Anthropic request failed: {err}")))?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "Anthropic provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        let response = extract_anthropic_response(&body)?;
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

fn build_headers(config: &AnthropicConfig) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        HeaderName::from_static("anthropic-version"),
        HeaderValue::from_static(ANTHROPIC_VERSION),
    );

    if let Some(api_key) = config
        .api_key
        .as_deref()
        .filter(|api_key| !api_key.trim().is_empty())
    {
        let value = HeaderValue::from_str(api_key)
            .map_err(|_| AstrbotError::Provider("invalid Anthropic API key header".to_string()))?;
        headers.insert(HeaderName::from_static("x-api-key"), value);
    }

    insert_custom_headers(&mut headers, &config.custom_headers)?;

    Ok(headers)
}
