use std::sync::{Arc, Mutex};

use astrbot_core::{
    AstrbotError, MessageChain, MessageEvent, MessageSender, MessageSession, MessageSink,
    ProviderContentPart, ProviderContextMessage, Result,
};
use astrbot_memory::{MemorySessionKey, MemoryTranscriptRecord};
use astrbot_provider::{ChatProvider, ChatRequest, ChatResponse};
use async_trait::async_trait;

use crate::{
    AgentHookEvent, AgentHookEventKind, AgentKnowledgeContextPort, AgentMemoryContextPort,
    AgentProviderPreferencePort, AgentQuoteContextPort, AgentRunHook, AgentSessionContextPort,
    AgentTokenCounter,
};
use astrbot_provider::{ProviderReasoningMetadata, ProviderResponseMetadata};

struct NoopSink;

#[async_trait]
impl MessageSink for NoopSink {
    async fn send(&self, _session: &MessageSession, _chain: MessageChain) -> Result<()> {
        Ok(())
    }
}

pub(super) fn event(text: impl Into<String>) -> MessageEvent {
    MessageEvent::new(
        "event-1",
        "webchat",
        "WebChat",
        MessageSession::new("webchat", "conversation-1"),
        MessageSender::new("user-1", None),
        MessageChain::plain(text),
        Arc::new(NoopSink),
    )
}

pub(super) fn group_event(text: impl Into<String>) -> MessageEvent {
    MessageEvent::new(
        "event-1",
        "webchat",
        "WebChat",
        MessageSession::group("webchat", "room-1"),
        MessageSender::new("user-1", Some("Alice".to_string())),
        MessageChain::plain(text),
        Arc::new(NoopSink),
    )
}

pub(super) struct StaticPreference;

#[async_trait]
impl AgentProviderPreferencePort for StaticPreference {
    async fn preferred_chat_provider_id(&self, _event: &MessageEvent) -> Result<Option<String>> {
        Ok(Some("preferred-provider".to_string()))
    }
}

pub(super) struct StaticSessionContext;

#[async_trait]
impl AgentSessionContextPort for StaticSessionContext {
    async fn context_messages(&self, _event: &MessageEvent) -> Result<Vec<ProviderContextMessage>> {
        Ok(vec![ProviderContextMessage::text("assistant", "previous")])
    }
}

pub(super) struct StaticQuoteContext;

#[async_trait]
impl AgentQuoteContextPort for StaticQuoteContext {
    async fn quote_content_parts(&self, _event: &MessageEvent) -> Result<Vec<ProviderContentPart>> {
        Ok(vec![ProviderContentPart::text("quoted")])
    }
}

pub(super) struct StaticMemoryContext;

#[async_trait]
impl AgentMemoryContextPort for StaticMemoryContext {
    async fn memory_records(&self, event: &MessageEvent) -> Result<Vec<MemoryTranscriptRecord>> {
        Ok(vec![MemoryTranscriptRecord::new(
            MemorySessionKey::from_session(&event.session),
            "Alice",
            "[Alice/12:00:00]: hello",
        )])
    }
}

pub(super) struct StaticKnowledgeContext;

#[async_trait]
impl AgentKnowledgeContextPort for StaticKnowledgeContext {
    async fn formatted_knowledge_context(&self, _event: &MessageEvent) -> Result<Option<String>> {
        Ok(Some("【知识 1】\n内容: Rust boundary".to_string()))
    }
}

pub(super) struct OneTokenPerMessageCounter;

impl AgentTokenCounter for OneTokenPerMessageCounter {
    fn count_text(&self, _text: &str) -> usize {
        1
    }

    fn count_message(&self, _message: &ProviderContextMessage) -> usize {
        1
    }
}

#[derive(Default)]
pub(super) struct CapturingHook {
    events: Mutex<Vec<AgentHookEvent>>,
}

impl CapturingHook {
    pub(super) fn kinds(&self) -> Vec<AgentHookEventKind> {
        self.events
            .lock()
            .expect("hook events should lock")
            .iter()
            .map(AgentHookEvent::kind)
            .collect()
    }

    pub(super) fn events(&self) -> Vec<AgentHookEvent> {
        self.events.lock().expect("hook events should lock").clone()
    }
}

#[async_trait]
impl AgentRunHook for CapturingHook {
    async fn on_event(&self, event: AgentHookEvent) -> Result<()> {
        self.events
            .lock()
            .expect("hook events should lock")
            .push(event);
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct CapturingProvider {
    pub(super) fail: bool,
    pub(super) reasoning_content: Option<String>,
}

#[async_trait]
impl ChatProvider for CapturingProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        if self.fail {
            return Err(AstrbotError::Provider("upstream failed".to_string()));
        }

        let response = ChatResponse::text(format!("{}:{}", request.session_id, request.prompt));
        if let Some(reasoning) = &self.reasoning_content {
            return Ok(response.with_metadata(
                ProviderResponseMetadata::default()
                    .with_reasoning(ProviderReasoningMetadata::new(reasoning.clone())),
            ));
        }

        Ok(response)
    }
}
