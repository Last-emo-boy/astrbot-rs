use astrbot_core::{MessageEvent, ProviderContextMessage, Result};
use async_trait::async_trait;

#[async_trait]
pub trait SessionStatusPort: Send + Sync {
    async fn is_session_enabled(&self, event: &MessageEvent) -> Result<bool>;
}

pub struct AllowAllSessionStatusPort;

#[async_trait]
impl SessionStatusPort for AllowAllSessionStatusPort {
    async fn is_session_enabled(&self, _event: &MessageEvent) -> Result<bool> {
        Ok(true)
    }
}

#[async_trait]
pub trait SessionContextPort: Send + Sync {
    async fn context_messages(&self, event: &MessageEvent) -> Result<Vec<ProviderContextMessage>>;
}

pub struct EmptySessionContextPort;

#[async_trait]
impl SessionContextPort for EmptySessionContextPort {
    async fn context_messages(&self, _event: &MessageEvent) -> Result<Vec<ProviderContextMessage>> {
        Ok(Vec::new())
    }
}
