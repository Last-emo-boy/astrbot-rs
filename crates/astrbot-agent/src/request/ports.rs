use astrbot_core::{MessageEvent, ProviderContentPart, ProviderContextMessage, Result};
use async_trait::async_trait;

#[async_trait]
pub trait AgentProviderPreferencePort: Send + Sync {
    async fn preferred_chat_provider_id(&self, event: &MessageEvent) -> Result<Option<String>>;
}

#[async_trait]
pub trait AgentSessionContextPort: Send + Sync {
    async fn context_messages(&self, event: &MessageEvent) -> Result<Vec<ProviderContextMessage>>;
}

#[async_trait]
pub trait AgentQuoteContextPort: Send + Sync {
    async fn quote_content_parts(&self, event: &MessageEvent) -> Result<Vec<ProviderContentPart>>;
}
