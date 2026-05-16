use std::collections::HashMap;
use std::sync::Arc;

use astrbot_core::{MessageEvent, Result};
use astrbot_storage::{
    InMemoryProviderPreferenceRepository, ProviderPreferenceRecord, ProviderPreferenceRepository,
};
use async_trait::async_trait;

#[async_trait]
pub trait ProviderPreferencePort: Send + Sync {
    async fn preferred_chat_provider_id(&self, event: &MessageEvent) -> Result<Option<String>>;
}

pub struct NoProviderPreferencePort;

#[async_trait]
impl ProviderPreferencePort for NoProviderPreferencePort {
    async fn preferred_chat_provider_id(&self, _event: &MessageEvent) -> Result<Option<String>> {
        Ok(None)
    }
}

pub struct InMemoryProviderPreferencePort {
    repository: Arc<dyn ProviderPreferenceRepository>,
}

impl Default for InMemoryProviderPreferencePort {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryProviderPreferencePort {
    pub fn new() -> Self {
        Self {
            repository: Arc::new(InMemoryProviderPreferenceRepository::new()),
        }
    }

    pub fn with_repository(repository: Arc<dyn ProviderPreferenceRepository>) -> Self {
        Self { repository }
    }

    pub async fn set_preferred_chat_provider(
        &self,
        session_id: impl Into<String>,
        provider_id: impl Into<String>,
    ) -> Result<()> {
        self.repository
            .set_preferred_chat_provider(ProviderPreferenceRecord::new(session_id, provider_id))
            .await
    }

    pub async fn snapshot(&self) -> Result<HashMap<String, String>> {
        self.repository.snapshot_chat_provider_preferences().await
    }

    pub async fn replace_with(&self, preferences: HashMap<String, String>) -> Result<()> {
        self.repository
            .replace_chat_provider_preferences(preferences)
            .await
    }
}

#[async_trait]
impl ProviderPreferencePort for InMemoryProviderPreferencePort {
    async fn preferred_chat_provider_id(&self, event: &MessageEvent) -> Result<Option<String>> {
        let platform_session = format!(
            "{}:{}",
            event.session.platform_id, event.session.conversation_id
        );
        self.repository
            .preferred_chat_provider_id(&platform_session, &event.session.conversation_id)
            .await
    }
}
