use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use astrbot_core::{
    AstrbotError, MessageComponent, MessageEvent, MessageSessionKind, ProviderRequest, Result,
};
use astrbot_memory::{
    ActiveReplyCheck, ActiveReplyPolicy, MemoryImageCaptionConfig, MemoryImageCaptionRequest,
    MemoryImageCaptioner, MemoryMessageInput, MemoryPromptPolicy, MemoryRequestMode,
    MemoryRetentionPolicy, MemorySessionKey, MemoryTranscriptBuilder, MemoryTranscriptRecord,
};
use astrbot_provider::{ChatProvider, ChatRequest, ChatResponse};
use async_trait::async_trait;

use crate::ProviderRequestDecorator;

#[async_trait]
pub trait AgentMemoryContextPort: Send + Sync {
    async fn memory_records(&self, event: &MessageEvent) -> Result<Vec<MemoryTranscriptRecord>>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentMemoryContextConfig {
    pub retention: MemoryRetentionPolicy,
}

impl AgentMemoryContextConfig {
    pub fn new(retention: MemoryRetentionPolicy) -> Self {
        Self { retention }
    }
}

impl Default for AgentMemoryContextConfig {
    fn default() -> Self {
        Self {
            retention: MemoryRetentionPolicy::default(),
        }
    }
}

pub struct InMemoryAgentMemoryContext {
    records: Mutex<HashMap<MemorySessionKey, Vec<MemoryTranscriptRecord>>>,
    builder: MemoryTranscriptBuilder,
    config: AgentMemoryContextConfig,
}

impl InMemoryAgentMemoryContext {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            builder: MemoryTranscriptBuilder::new(),
            config: AgentMemoryContextConfig::default(),
        }
    }

    pub fn with_config(mut self, config: AgentMemoryContextConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_retention_policy(mut self, retention: MemoryRetentionPolicy) -> Self {
        self.config.retention = retention;
        self
    }

    pub fn with_captioner(
        mut self,
        captioner: Arc<dyn MemoryImageCaptioner>,
        caption_config: MemoryImageCaptionConfig,
    ) -> Self {
        self.builder = MemoryTranscriptBuilder::new().with_captioner(captioner, caption_config);
        self
    }

    pub async fn record_message(
        &self,
        event: &MessageEvent,
    ) -> Result<Option<MemoryTranscriptRecord>> {
        self.record_message_inner(event, None).await
    }

    pub async fn record_message_with_timestamp(
        &self,
        event: &MessageEvent,
        timestamp: impl Into<String>,
    ) -> Result<Option<MemoryTranscriptRecord>> {
        self.record_message_inner(event, Some(timestamp.into()))
            .await
    }

    pub async fn record_response(
        &self,
        event: &MessageEvent,
        response: &ChatResponse,
    ) -> Result<Option<MemoryTranscriptRecord>> {
        self.record_response_text(event, response.chain.plain_text())
            .await
    }

    pub async fn record_response_text(
        &self,
        event: &MessageEvent,
        text: impl Into<String>,
    ) -> Result<Option<MemoryTranscriptRecord>> {
        self.record_response_text_inner(event, text.into(), None)
            .await
    }

    pub async fn record_response_text_with_timestamp(
        &self,
        event: &MessageEvent,
        text: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Result<Option<MemoryTranscriptRecord>> {
        self.record_response_text_inner(event, text.into(), Some(timestamp.into()))
            .await
    }

    pub fn remove_session(&self, event: &MessageEvent) -> Result<usize> {
        self.remove_session_key(&MemorySessionKey::from_session(&event.session))
    }

    pub fn remove_session_key(&self, key: &MemorySessionKey) -> Result<usize> {
        let mut records = self.lock_records()?;
        Ok(records.remove(key).map(|items| items.len()).unwrap_or(0))
    }

    pub fn after_message_sent_cleanup(
        &self,
        event: &MessageEvent,
        clean_session: bool,
    ) -> Result<usize> {
        if clean_session {
            self.remove_session(event)
        } else {
            Ok(0)
        }
    }

    fn memory_records_for_key(
        &self,
        key: &MemorySessionKey,
    ) -> Result<Vec<MemoryTranscriptRecord>> {
        let records = self.lock_records()?;
        Ok(records.get(key).cloned().unwrap_or_default())
    }

    async fn record_message_inner(
        &self,
        event: &MessageEvent,
        timestamp: Option<String>,
    ) -> Result<Option<MemoryTranscriptRecord>> {
        if event.session.kind != MessageSessionKind::Group {
            return Ok(None);
        }

        let Some(input) = memory_input_from_event(event, timestamp) else {
            return Ok(None);
        };
        let Some(record) = self.builder.build(input).await? else {
            return Ok(None);
        };
        self.append_record(record.clone())?;
        Ok(Some(record))
    }

    async fn record_response_text_inner(
        &self,
        event: &MessageEvent,
        text: String,
        timestamp: Option<String>,
    ) -> Result<Option<MemoryTranscriptRecord>> {
        if text.trim().is_empty() {
            return Ok(None);
        }
        let session = MemorySessionKey::from_session(&event.session);
        if self.memory_records_for_key(&session)?.is_empty() {
            return Ok(None);
        }

        let mut input = MemoryMessageInput::new(session, "You").with_text(text);
        if let Some(timestamp) = timestamp {
            input = input.with_timestamp(timestamp);
        }
        let Some(record) = self.builder.build(input).await? else {
            return Ok(None);
        };
        self.append_record(record.clone())?;
        Ok(Some(record))
    }

    fn append_record(&self, record: MemoryTranscriptRecord) -> Result<()> {
        let mut records = self.lock_records()?;
        let session_records = records.entry(record.session.clone()).or_default();
        session_records.push(record);
        self.config.retention.apply(session_records);
        Ok(())
    }

    fn lock_records(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<MemorySessionKey, Vec<MemoryTranscriptRecord>>>>
    {
        self.records.lock().map_err(|_| {
            AstrbotError::Pipeline("memory transcript store lock poisoned".to_string())
        })
    }
}

impl Default for InMemoryAgentMemoryContext {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentMemoryContextPort for InMemoryAgentMemoryContext {
    async fn memory_records(&self, event: &MessageEvent) -> Result<Vec<MemoryTranscriptRecord>> {
        self.memory_records_for_key(&MemorySessionKey::from_session(&event.session))
    }
}

pub struct ChatProviderMemoryImageCaptioner {
    provider: Arc<dyn ChatProvider>,
}

impl ChatProviderMemoryImageCaptioner {
    pub fn new(provider: Arc<dyn ChatProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl MemoryImageCaptioner for ChatProviderMemoryImageCaptioner {
    async fn caption_image(&self, request: MemoryImageCaptionRequest) -> Result<Option<String>> {
        let mut chat_request =
            ChatRequest::new(request.prompt, request.session_id).with_image_url(request.image_url);
        if let Some(provider_id) = request.provider_id {
            chat_request = chat_request.with_provider_id(provider_id);
        }
        let response = self.provider.chat(chat_request).await?;
        let caption = response.chain.plain_text().trim().to_string();
        Ok((!caption.is_empty()).then_some(caption))
    }
}

pub struct MemoryRequestDecorator {
    memory: Arc<dyn AgentMemoryContextPort>,
    prompt_policy: MemoryPromptPolicy,
    mode: MemoryRequestMode,
}

impl MemoryRequestDecorator {
    pub fn new(memory: Arc<dyn AgentMemoryContextPort>) -> Self {
        Self {
            memory,
            prompt_policy: MemoryPromptPolicy::default(),
            mode: MemoryRequestMode::PassiveContext,
        }
    }

    pub fn active_reply(mut self) -> Self {
        self.mode = MemoryRequestMode::ActiveReply;
        self
    }

    pub fn with_prompt_policy(mut self, prompt_policy: MemoryPromptPolicy) -> Self {
        self.prompt_policy = prompt_policy;
        self
    }
}

#[async_trait]
impl ProviderRequestDecorator for MemoryRequestDecorator {
    async fn decorate(&self, event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        let records = self.memory.memory_records(event).await?;
        let Some(plan) = self.prompt_policy.build_plan(&records, self.mode) else {
            return Ok(());
        };
        plan.apply_to_request(request);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentActiveReplyDecider {
    policy: ActiveReplyPolicy,
}

impl AgentActiveReplyDecider {
    pub fn new(policy: ActiveReplyPolicy) -> Self {
        Self { policy }
    }

    pub fn should_reply(&self, event: &MessageEvent, roll: f32) -> bool {
        self.policy.should_reply(&ActiveReplyCheck {
            session: MemorySessionKey::from_session(&event.session),
            session_kind: event.session.kind,
            is_at_or_wake_command: event.is_at_or_wake_command(),
            roll,
            recent_message_count: 1,
            window_seconds: None,
            seconds_since_last_reply: None,
        })
    }
}

fn memory_input_from_event(
    event: &MessageEvent,
    timestamp: Option<String>,
) -> Option<MemoryMessageInput> {
    let mut input = MemoryMessageInput::new(
        MemorySessionKey::from_session(&event.session),
        speaker_label(event),
    );
    let mut has_supported_part = false;

    for component in event.message.components() {
        match component {
            MessageComponent::Plain { text } => {
                if !text.trim().is_empty() {
                    input = input.with_text(text.clone());
                    has_supported_part = true;
                }
            }
            MessageComponent::Image { url } => {
                if !url.trim().is_empty() {
                    input = input.with_image_url(url.clone());
                    has_supported_part = true;
                }
            }
            MessageComponent::Mention { user_id } => {
                if !user_id.trim().is_empty() {
                    input = input.with_mention(user_id.clone());
                    has_supported_part = true;
                }
            }
            MessageComponent::MentionAll => {
                input = input.with_mention("all");
                has_supported_part = true;
            }
            MessageComponent::Record { .. }
            | MessageComponent::Video { .. }
            | MessageComponent::File { .. }
            | MessageComponent::Reply { .. } => {}
        }
    }

    if let Some(timestamp) = timestamp {
        input = input.with_timestamp(timestamp);
    }

    has_supported_part.then_some(input)
}

fn speaker_label(event: &MessageEvent) -> String {
    event
        .sender
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(&event.sender.id)
        .to_string()
}
