use std::collections::HashMap;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use crate::http::{build_http_client, extract_error_message, join_api_path, json_bearer_headers};
use crate::protocol::openai_chat::{
    OpenAiChatCompletionRequest, build_openai_chat_completion_request,
    collect_openai_streaming_content, extract_openai_message_content,
};
use crate::{ChatProvider, ChatRequest, ChatResponse};

#[derive(Clone, Debug)]
pub struct OpenAiCompatibleConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
}

impl OpenAiCompatibleConfig {
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

    fn chat_completions_url(&self) -> String {
        join_api_path(&self.api_base, "chat/completions")
    }
}

#[derive(Clone, Debug)]
pub struct OpenAiCompatibleProvider {
    config: OpenAiCompatibleConfig,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: OpenAiCompatibleConfig) -> Result<Self> {
        let client = build_http_client(
            config.timeout,
            json_bearer_headers(
                config.api_key.as_deref(),
                &config.custom_headers,
                "invalid OpenAI-compatible API key header",
            )?,
        )?;

        Ok(Self { config, client })
    }

    fn build_payload(&self, request: &ChatRequest) -> OpenAiChatCompletionRequest {
        build_openai_chat_completion_request(request, &self.config.model)
    }
}

#[async_trait]
impl ChatProvider for OpenAiCompatibleProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let response = self
            .client
            .post(self.config.chat_completions_url())
            .json(&self.build_payload(&request))
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("OpenAI-compatible request failed: {err}"))
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "OpenAI-compatible provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        if request.stream {
            let content = collect_openai_streaming_content(body.as_str())?;
            if content.trim().is_empty() {
                return Err(AstrbotError::Provider(
                    "provider stream did not contain assistant content".to_string(),
                ));
            }
            return Ok(ChatResponse::text(content));
        }

        let content = extract_openai_message_content(&body)?;

        if content.trim().is_empty() {
            return Err(AstrbotError::Provider(
                "provider response did not contain assistant content".to_string(),
            ));
        }

        Ok(ChatResponse::text(content))
    }
}
