mod event;

use std::sync::Arc;

use astrbot_core::Result;
use async_trait::async_trait;

pub use event::{
    AgentDoneEvent, AgentHookEvent, AgentHookEventKind, AgentLifecycleEvent,
    AgentToolLifecycleEvent,
};

#[async_trait]
pub trait AgentRunHook: Send + Sync {
    async fn on_event(&self, event: AgentHookEvent) -> Result<()>;
}

#[derive(Default)]
pub struct NoopAgentRunHook;

#[async_trait]
impl AgentRunHook for NoopAgentRunHook {
    async fn on_event(&self, _event: AgentHookEvent) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct CompositeAgentRunHook {
    hooks: Vec<Arc<dyn AgentRunHook>>,
}

impl CompositeAgentRunHook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_hook(mut self, hook: Arc<dyn AgentRunHook>) -> Self {
        self.hooks.push(hook);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }
}

#[async_trait]
impl AgentRunHook for CompositeAgentRunHook {
    async fn on_event(&self, event: AgentHookEvent) -> Result<()> {
        for hook in &self.hooks {
            hook.on_event(event.clone()).await?;
        }
        Ok(())
    }
}
