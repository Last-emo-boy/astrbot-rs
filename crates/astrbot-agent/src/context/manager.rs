use std::sync::Arc;

use astrbot_core::{ProviderContextMessage, Result};

use super::{
    AgentContextCompressor, AgentContextWindow, AgentTokenCounter, ApproximateTokenCounter,
    ContextTokenBudget, ContextTruncationPolicy, NoopContextCompressor,
};

pub struct ContextWindowManager {
    token_counter: Arc<dyn AgentTokenCounter>,
    token_budget: ContextTokenBudget,
    truncation_policy: ContextTruncationPolicy,
    compressor: Arc<dyn AgentContextCompressor>,
}

impl ContextWindowManager {
    pub fn new(token_budget: ContextTokenBudget) -> Self {
        Self {
            token_counter: Arc::new(ApproximateTokenCounter),
            token_budget,
            truncation_policy: ContextTruncationPolicy::default(),
            compressor: Arc::new(NoopContextCompressor),
        }
    }

    pub fn with_token_counter(mut self, token_counter: Arc<dyn AgentTokenCounter>) -> Self {
        self.token_counter = token_counter;
        self
    }

    pub fn with_truncation_policy(mut self, truncation_policy: ContextTruncationPolicy) -> Self {
        self.truncation_policy = truncation_policy;
        self
    }

    pub fn with_compressor(mut self, compressor: Arc<dyn AgentContextCompressor>) -> Self {
        self.compressor = compressor;
        self
    }

    pub fn token_budget(&self) -> &ContextTokenBudget {
        &self.token_budget
    }

    pub async fn prepare_messages(
        &self,
        messages: Vec<ProviderContextMessage>,
    ) -> Result<Vec<ProviderContextMessage>> {
        Ok(self
            .prepare_window(AgentContextWindow::from_messages(messages))
            .await?
            .into_messages())
    }

    pub async fn prepare_window(&self, window: AgentContextWindow) -> Result<AgentContextWindow> {
        let mut window = self.truncation_policy.truncate_by_message_count(window);

        let Some(max_tokens) = self.token_budget.available_context_tokens() else {
            return Ok(window);
        };

        let current_tokens = window.total_tokens(self.token_counter.as_ref());
        if self
            .compressor
            .should_compress(&window, current_tokens, &self.token_budget)
        {
            window = self
                .compressor
                .compress(window, &self.token_budget, self.token_counter.as_ref())
                .await?;
        }

        if window.total_tokens(self.token_counter.as_ref()) > max_tokens {
            window = self.truncation_policy.truncate_to_token_budget(
                window,
                &self.token_budget,
                self.token_counter.as_ref(),
            );
        }

        Ok(window)
    }
}

impl Default for ContextWindowManager {
    fn default() -> Self {
        Self::new(ContextTokenBudget::default())
    }
}
