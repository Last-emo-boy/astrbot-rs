use std::collections::HashMap;
use std::sync::Arc;

use astrbot_core::Result;
use astrbot_session::{ProviderCapability, SessionProviderPreference, SessionRuleKey};
use async_trait::async_trait;

use crate::{InMemorySessionRuleRepository, SessionRuleRepository};

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

pub struct InMemoryProviderPreferenceRepository {
    session_rules: Arc<dyn SessionRuleRepository>,
}

impl InMemoryProviderPreferenceRepository {
    pub fn new() -> Self {
        Self {
            session_rules: Arc::new(InMemorySessionRuleRepository::new()),
        }
    }

    pub fn with_session_rules(session_rules: Arc<dyn SessionRuleRepository>) -> Self {
        Self { session_rules }
    }
}

impl Default for InMemoryProviderPreferenceRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderPreferenceRepository for InMemoryProviderPreferenceRepository {
    async fn preferred_chat_provider_id(
        &self,
        platform_session_id: &str,
        conversation_id: &str,
    ) -> Result<Option<String>> {
        if let Some(provider_id) = self
            .session_rules
            .provider_preference(platform_session_id, ProviderCapability::ChatCompletion)
            .await?
        {
            return Ok(Some(provider_id));
        }

        self.session_rules
            .provider_preference(conversation_id, ProviderCapability::ChatCompletion)
            .await
    }

    async fn set_preferred_chat_provider(&self, record: ProviderPreferenceRecord) -> Result<()> {
        let session_id = record.session_id.trim().to_string();
        let provider_id = record.provider_id.trim().to_string();
        if session_id.is_empty() || provider_id.is_empty() {
            return Ok(());
        }

        self.session_rules
            .set_provider_preference(
                &session_id,
                SessionProviderPreference::new(ProviderCapability::ChatCompletion, provider_id)
                    .expect("provider id was validated"),
            )
            .await
    }

    async fn snapshot_chat_provider_preferences(&self) -> Result<HashMap<String, String>> {
        let mut preferences = HashMap::new();
        for rule_set in self.session_rules.list_rule_sets().await? {
            if let Some(provider_id) = rule_set.provider_for(ProviderCapability::ChatCompletion) {
                preferences.insert(rule_set.umo.clone(), provider_id.to_string());
            }
        }
        Ok(preferences)
    }

    async fn replace_chat_provider_preferences(
        &self,
        preferences: HashMap<String, String>,
    ) -> Result<()> {
        for rule_set in self.session_rules.list_rule_sets().await? {
            self.session_rules
                .delete_rule(
                    &rule_set.umo,
                    SessionRuleKey::Provider(ProviderCapability::ChatCompletion),
                )
                .await?;
        }

        for (session_id, provider_id) in preferences {
            self.set_preferred_chat_provider(ProviderPreferenceRecord::new(
                session_id,
                provider_id,
            ))
            .await?;
        }
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
