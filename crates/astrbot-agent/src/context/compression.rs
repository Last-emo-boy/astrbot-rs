use astrbot_core::Result;
use async_trait::async_trait;

use super::{AgentContextWindow, AgentTokenCounter, ContextTokenBudget};

#[async_trait]
pub trait AgentContextCompressor: Send + Sync {
    fn should_compress(
        &self,
        _window: &AgentContextWindow,
        current_tokens: usize,
        budget: &ContextTokenBudget,
    ) -> bool {
        budget
            .available_context_tokens()
            .is_some_and(|max_tokens| current_tokens > max_tokens)
    }

    async fn compress(
        &self,
        window: AgentContextWindow,
        budget: &ContextTokenBudget,
        counter: &dyn AgentTokenCounter,
    ) -> Result<AgentContextWindow>;
}

#[derive(Clone, Debug, Default)]
pub struct NoopContextCompressor;

#[async_trait]
impl AgentContextCompressor for NoopContextCompressor {
    fn should_compress(
        &self,
        _window: &AgentContextWindow,
        _current_tokens: usize,
        _budget: &ContextTokenBudget,
    ) -> bool {
        false
    }

    async fn compress(
        &self,
        window: AgentContextWindow,
        _budget: &ContextTokenBudget,
        _counter: &dyn AgentTokenCounter,
    ) -> Result<AgentContextWindow> {
        Ok(window)
    }
}
