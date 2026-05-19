use std::collections::BTreeMap;

use astrbot_core::{MessageChain, MessageEventResult, ProviderRequest, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{AgentFeedbackEvent, AgentFeedbackEventKind};

pub mod agent_clients;

pub use agent_clients::{
    CozeAgentClient, CozeChatStarted, DashScopeAgentClient, DifyAgentClient, parse_dify_sse_event,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalAgentConnectorKind {
    Coze,
    Dify,
    DashScope,
    DeerFlow,
    Custom(String),
}

impl ExternalAgentConnectorKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Coze => "coze",
            Self::Dify => "dify",
            Self::DashScope => "dashscope",
            Self::DeerFlow => "deerflow",
            Self::Custom(kind) => kind,
        }
    }
}

impl From<&str> for ExternalAgentConnectorKind {
    fn from(kind: &str) -> Self {
        match kind.trim() {
            "coze" => Self::Coze,
            "dify" => Self::Dify,
            "dashscope" => Self::DashScope,
            "deerflow" => Self::DeerFlow,
            other => Self::Custom(other.to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalAgentConnectorConfig {
    pub connector_id: String,
    pub kind: ExternalAgentConnectorKind,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub app_id: Option<String>,
    #[serde(default)]
    pub bot_id: Option<String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
}

impl ExternalAgentConnectorConfig {
    pub fn new(connector_id: impl Into<String>, kind: ExternalAgentConnectorKind) -> Self {
        Self {
            connector_id: connector_id.into(),
            kind,
            api_base: None,
            api_key: None,
            app_id: None,
            bot_id: None,
            timeout_secs: default_timeout_secs(),
            stream: false,
            options: BTreeMap::new(),
        }
    }

    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = non_empty_option(api_base);
        self
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = non_empty_option(api_key);
        self
    }

    pub fn with_app_id(mut self, app_id: impl Into<String>) -> Self {
        self.app_id = non_empty_option(app_id);
        self
    }

    pub fn with_bot_id(mut self, bot_id: impl Into<String>) -> Self {
        self.bot_id = non_empty_option(bot_id);
        self
    }

    pub fn with_streaming(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        if !key.trim().is_empty() && !value.trim().is_empty() {
            self.options
                .insert(key.trim().to_string(), value.trim().to_string());
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalAgentRequest {
    pub connector_id: String,
    pub session_id: String,
    pub prompt: String,
    pub stream: bool,
    pub image_urls: Vec<String>,
    pub system_prompt: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

impl ExternalAgentRequest {
    pub fn from_provider_request(
        config: &ExternalAgentConnectorConfig,
        request: &ProviderRequest,
    ) -> Self {
        let mut metadata = BTreeMap::new();
        metadata.insert("kind".to_string(), config.kind.as_str().to_string());
        if let Some(app_id) = &config.app_id {
            metadata.insert("app_id".to_string(), app_id.clone());
        }
        if let Some(bot_id) = &config.bot_id {
            metadata.insert("bot_id".to_string(), bot_id.clone());
        }

        Self {
            connector_id: config.connector_id.clone(),
            session_id: request.session_id.clone().unwrap_or_default(),
            prompt: request.prompt.clone().unwrap_or_default(),
            stream: request.stream || config.stream,
            image_urls: request.image_urls.clone(),
            system_prompt: request.system_prompt.clone(),
            metadata,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExternalAgentRunStateKind {
    #[default]
    Idle,
    Running,
    Done,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalAgentRunState {
    pub kind: ExternalAgentRunStateKind,
    pub session_id: String,
    pub remote_thread_id: Option<String>,
}

impl ExternalAgentRunState {
    pub fn running(session_id: impl Into<String>) -> Self {
        Self {
            kind: ExternalAgentRunStateKind::Running,
            session_id: session_id.into(),
            remote_thread_id: None,
        }
    }

    pub fn with_remote_thread_id(mut self, remote_thread_id: impl Into<String>) -> Self {
        self.remote_thread_id = non_empty_option(remote_thread_id);
        self
    }
}

#[async_trait]
pub trait ExternalAgentConnector: Send + Sync {
    async fn reset(&self, request: ExternalAgentRequest) -> Result<ExternalAgentRunState>;

    async fn next_events(
        &self,
        state: &mut ExternalAgentRunState,
    ) -> Result<Vec<ExternalAgentRawStreamEvent>>;

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalAgentRawStreamEvent {
    pub event_type: String,
    pub text_delta: Option<String>,
    pub final_text: Option<String>,
    pub error: Option<String>,
    pub remote_thread_id: Option<String>,
}

impl ExternalAgentRawStreamEvent {
    pub fn delta(event_type: impl Into<String>, text_delta: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            text_delta: non_blank_text(text_delta),
            final_text: None,
            error: None,
            remote_thread_id: None,
        }
    }

    pub fn final_text(event_type: impl Into<String>, final_text: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            text_delta: None,
            final_text: non_blank_text(final_text),
            error: None,
            remote_thread_id: None,
        }
    }

    pub fn error(event_type: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            text_delta: None,
            final_text: None,
            error: non_blank_text(error),
            remote_thread_id: None,
        }
    }

    pub fn with_remote_thread_id(mut self, remote_thread_id: impl Into<String>) -> Self {
        self.remote_thread_id = non_empty_option(remote_thread_id);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalAgentMappedEvent {
    pub feedback_event: Option<AgentFeedbackEvent>,
    pub final_result: Option<MessageEventResult>,
    pub remote_thread_id: Option<String>,
    pub state: ExternalAgentRunStateKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExternalAgentStreamMapper {
    accumulated_text: String,
}

impl ExternalAgentStreamMapper {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn accumulated_text(&self) -> &str {
        &self.accumulated_text
    }

    pub fn map_event(&mut self, event: ExternalAgentRawStreamEvent) -> ExternalAgentMappedEvent {
        if let Some(error) = event.error {
            return ExternalAgentMappedEvent {
                feedback_event: Some(AgentFeedbackEvent::new(
                    AgentFeedbackEventKind::Error,
                    MessageChain::plain(error.clone()),
                )),
                final_result: Some(MessageEventResult::general(error)),
                remote_thread_id: event.remote_thread_id,
                state: ExternalAgentRunStateKind::Error,
            };
        }

        if let Some(delta) = event.text_delta {
            self.accumulated_text.push_str(&delta);
            return ExternalAgentMappedEvent {
                feedback_event: Some(AgentFeedbackEvent::streaming_delta(delta)),
                final_result: None,
                remote_thread_id: event.remote_thread_id,
                state: ExternalAgentRunStateKind::Running,
            };
        }

        if event.event_type.contains("completed") || event.final_text.is_some() {
            let final_text = event
                .final_text
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_else(|| self.accumulated_text.clone());
            return ExternalAgentMappedEvent {
                feedback_event: None,
                final_result: (!final_text.trim().is_empty())
                    .then(|| MessageEventResult::llm(MessageChain::plain(final_text))),
                remote_thread_id: event.remote_thread_id,
                state: ExternalAgentRunStateKind::Done,
            };
        }

        ExternalAgentMappedEvent {
            feedback_event: None,
            final_result: None,
            remote_thread_id: event.remote_thread_id,
            state: ExternalAgentRunStateKind::Running,
        }
    }
}

fn default_timeout_secs() -> u64 {
    120
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then(|| value.trim().to_string())
}

fn non_blank_text(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use astrbot_core::ProviderRequest;

    use super::{
        ExternalAgentConnectorConfig, ExternalAgentConnectorKind, ExternalAgentRawStreamEvent,
        ExternalAgentRequest, ExternalAgentRunStateKind, ExternalAgentStreamMapper,
    };

    #[test]
    fn external_agent_request_adapts_provider_request_without_chat_provider_config() {
        let config =
            ExternalAgentConnectorConfig::new("coze-main", ExternalAgentConnectorKind::Coze)
                .with_app_id("app-1")
                .with_bot_id("bot-1")
                .with_streaming(true);
        let request = ProviderRequest::new("hello", "session-1")
            .with_image_url("image.png")
            .with_system_prompt("persona");

        let external = ExternalAgentRequest::from_provider_request(&config, &request);

        assert_eq!(external.connector_id, "coze-main");
        assert_eq!(external.session_id, "session-1");
        assert_eq!(external.prompt, "hello");
        assert!(external.stream);
        assert_eq!(external.image_urls, vec!["image.png"]);
        assert_eq!(
            external.metadata.get("kind").map(String::as_str),
            Some("coze")
        );
        assert_eq!(
            external.metadata.get("bot_id").map(String::as_str),
            Some("bot-1")
        );
    }

    #[test]
    fn stream_mapper_normalizes_deltas_final_text_and_errors() {
        let mut mapper = ExternalAgentStreamMapper::new();
        let first = mapper.map_event(
            ExternalAgentRawStreamEvent::delta("conversation.message.delta", "hello ")
                .with_remote_thread_id("thread-1"),
        );
        assert_eq!(first.state, ExternalAgentRunStateKind::Running);
        assert_eq!(
            first
                .feedback_event
                .expect("delta feedback should exist")
                .chain
                .plain_text(),
            "hello "
        );

        mapper.map_event(ExternalAgentRawStreamEvent::delta(
            "conversation.message.delta",
            "world",
        ));
        let final_event = mapper.map_event(ExternalAgentRawStreamEvent::final_text(
            "conversation.chat.completed",
            "",
        ));

        assert_eq!(final_event.state, ExternalAgentRunStateKind::Done);
        assert_eq!(
            final_event
                .final_result
                .expect("final result should exist")
                .chain
                .plain_text(),
            "hello world"
        );

        let error = mapper.map_event(ExternalAgentRawStreamEvent::error("error", "failed"));
        assert_eq!(error.state, ExternalAgentRunStateKind::Error);
        assert!(error.final_result.is_some());
    }
}
