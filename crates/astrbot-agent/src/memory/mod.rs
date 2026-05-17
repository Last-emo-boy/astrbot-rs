use std::sync::Arc;

use astrbot_core::{MessageEvent, ProviderRequest, Result};
use astrbot_memory::{
    ActiveReplyCheck, ActiveReplyPolicy, MemoryPromptPolicy, MemoryRequestMode, MemorySessionKey,
    MemoryTranscriptRecord,
};
use async_trait::async_trait;

use crate::ProviderRequestDecorator;

#[async_trait]
pub trait AgentMemoryContextPort: Send + Sync {
    async fn memory_records(&self, event: &MessageEvent) -> Result<Vec<MemoryTranscriptRecord>>;
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
        })
    }
}
