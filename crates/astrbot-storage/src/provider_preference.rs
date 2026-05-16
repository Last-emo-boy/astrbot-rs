use std::collections::HashMap;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderPreferenceRecord {
    pub session_id: String,
    pub provider_id: String,
}

impl ProviderPreferenceRecord {
    pub fn new(session_id: impl Into<String>, provider_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            provider_id: provider_id.into(),
        }
    }
}

#[async_trait]
pub trait ProviderPreferenceRepository: Send + Sync {
    async fn preferred_chat_provider_id(
        &self,
        platform_session_id: &str,
        conversation_id: &str,
    ) -> Result<Option<String>>;

    async fn set_preferred_chat_provider(&self, record: ProviderPreferenceRecord) -> Result<()>;

    async fn snapshot_chat_provider_preferences(&self) -> Result<HashMap<String, String>>;

    async fn replace_chat_provider_preferences(
        &self,
        preferences: HashMap<String, String>,
    ) -> Result<()>;
}

#[derive(Default)]
pub struct InMemoryProviderPreferenceRepository {
    chat_provider_ids: RwLock<HashMap<String, String>>,
}

impl InMemoryProviderPreferenceRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ProviderPreferenceRepository for InMemoryProviderPreferenceRepository {
    async fn preferred_chat_provider_id(
        &self,
        platform_session_id: &str,
        conversation_id: &str,
    ) -> Result<Option<String>> {
        let preferences = self
            .chat_provider_ids
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("provider preference lock: {err}")))?;
        Ok(preferences
            .get(platform_session_id)
            .or_else(|| preferences.get(conversation_id))
            .cloned())
    }

    async fn set_preferred_chat_provider(&self, record: ProviderPreferenceRecord) -> Result<()> {
        let session_id = record.session_id.trim().to_string();
        let provider_id = record.provider_id.trim().to_string();
        if session_id.is_empty() || provider_id.is_empty() {
            return Ok(());
        }

        self.chat_provider_ids
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("provider preference lock: {err}")))?
            .insert(session_id, provider_id);
        Ok(())
    }

    async fn snapshot_chat_provider_preferences(&self) -> Result<HashMap<String, String>> {
        self.chat_provider_ids
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("provider preference lock: {err}")))
            .map(|preferences| preferences.clone())
    }

    async fn replace_chat_provider_preferences(
        &self,
        preferences: HashMap<String, String>,
    ) -> Result<()> {
        let mut current = self
            .chat_provider_ids
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("provider preference lock: {err}")))?;
        *current = preferences;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InMemoryProviderPreferenceRepository, ProviderPreferenceRecord,
        ProviderPreferenceRepository,
    };

    #[tokio::test]
    async fn provider_preference_prefers_platform_session_over_conversation() {
        let repository = InMemoryProviderPreferenceRepository::new();
        repository
            .set_preferred_chat_provider(ProviderPreferenceRecord::new(
                "conversation-1",
                "fallback",
            ))
            .await
            .expect("preference should store");
        repository
            .set_preferred_chat_provider(ProviderPreferenceRecord::new(
                "webchat:conversation-1",
                "platform-specific",
            ))
            .await
            .expect("preference should store");

        let preferred = repository
            .preferred_chat_provider_id("webchat:conversation-1", "conversation-1")
            .await
            .expect("preference should load");

        assert_eq!(preferred.as_deref(), Some("platform-specific"));
    }
}
