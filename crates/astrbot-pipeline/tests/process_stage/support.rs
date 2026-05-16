use std::sync::Arc;

use astrbot_core::{
    AstrbotError, MessageChain, MessageEvent, MessageEventResult, MessageSender, MessageSession,
    ProviderContentPart, ProviderContextMessage, ProviderRequest, ProviderToolPlaceholder, Result,
};
use astrbot_pipeline::SessionContextPort;
use astrbot_platform::RecordingSink;
use astrbot_plugin::{PluginControl, PluginHandler};
use astrbot_provider::{ChatProvider, ChatRequest, ChatResponse};
use async_trait::async_trait;
use tokio::sync::Mutex;

pub fn direct_event(text: impl Into<String>, sink: Arc<RecordingSink>) -> MessageEvent {
    event_with_chain(MessageChain::plain(text), sink)
}

pub fn event_with_chain(message: MessageChain, sink: Arc<RecordingSink>) -> MessageEvent {
    MessageEvent::new(
        "event-1",
        "mock",
        "Mock Platform",
        MessageSession::new("mock", "conversation-1"),
        MessageSender::new("user-1", None),
        message,
        sink,
    )
}

#[derive(Default)]
pub struct CapturingProvider {
    pub requests: Mutex<Vec<ChatRequest>>,
}

#[async_trait]
impl ChatProvider for CapturingProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        self.requests.lock().await.push(request);
        Ok(ChatResponse::text("mock-response"))
    }
}

pub struct FailingProvider;

#[async_trait]
impl ChatProvider for FailingProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse> {
        Err(AstrbotError::Provider(
            "upstream returned secret details".to_string(),
        ))
    }
}

pub struct StaticSessionContextPort {
    contexts: Vec<ProviderContextMessage>,
}

impl StaticSessionContextPort {
    pub fn new(contexts: Vec<ProviderContextMessage>) -> Self {
        Self { contexts }
    }
}

#[async_trait]
impl SessionContextPort for StaticSessionContextPort {
    async fn context_messages(&self, _event: &MessageEvent) -> Result<Vec<ProviderContextMessage>> {
        Ok(self.contexts.clone())
    }
}

pub struct StaticReplyHandler {
    pub reply: &'static str,
}

#[async_trait]
impl PluginHandler for StaticReplyHandler {
    async fn handle(&self, event: &mut MessageEvent) -> Result<PluginControl> {
        event.set_result(MessageEventResult::general(self.reply));
        Ok(PluginControl::Continue)
    }
}

pub struct NoopHandler;

#[async_trait]
impl PluginHandler for NoopHandler {
    async fn handle(&self, _event: &mut MessageEvent) -> Result<PluginControl> {
        Ok(PluginControl::Continue)
    }
}

pub struct ProviderRequestHandler;

#[async_trait]
impl PluginHandler for ProviderRequestHandler {
    async fn handle(&self, event: &mut MessageEvent) -> Result<PluginControl> {
        event.set_provider_request(
            ProviderRequest::new("plugin prompt", "plugin-session")
                .with_provider_id("configured-provider")
                .with_image_url("https://example.test/plugin.png")
                .with_system_prompt("system from plugin")
                .with_model("plugin-model")
                .with_wake_prefix("llm")
                .with_context(ProviderContextMessage::text("assistant", "previous"))
                .with_extra_user_content_part(ProviderContentPart::text("extra instruction"))
                .with_tool_placeholder(ProviderToolPlaceholder::new("search")),
        );
        Ok(PluginControl::Continue)
    }
}
