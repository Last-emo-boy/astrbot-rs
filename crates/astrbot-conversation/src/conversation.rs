use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationRecord {
    pub platform_id: String,
    pub conversation_id: String,
    pub title: Option<String>,
    pub persona_id: Option<String>,
}

impl ConversationRecord {
    pub fn new(platform_id: impl Into<String>, conversation_id: impl Into<String>) -> Self {
        Self {
            platform_id: platform_id.into(),
            conversation_id: conversation_id.into(),
            title: None,
            persona_id: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_persona_id(mut self, persona_id: impl Into<String>) -> Self {
        self.persona_id = Some(persona_id.into());
        self
    }
}

#[async_trait]
pub trait ConversationDirectory: Send + Sync {
    async fn upsert_conversation(&self, record: ConversationRecord) -> Result<()>;

    async fn conversation(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ConversationRecord>>;

    async fn delete_conversation(&self, platform_id: &str, conversation_id: &str) -> Result<bool>;

    async fn set_current_conversation(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<()>;

    async fn current_conversation(&self, platform_id: &str) -> Result<Option<ConversationRecord>>;
}

#[derive(Default)]
pub struct InMemoryConversationDirectory {
    conversations: RwLock<HashMap<ConversationKey, ConversationRecord>>,
    current_by_platform: RwLock<HashMap<String, ConversationKey>>,
}

impl InMemoryConversationDirectory {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ConversationDirectory for InMemoryConversationDirectory {
    async fn upsert_conversation(&self, record: ConversationRecord) -> Result<()> {
        let key = ConversationKey::new(&record.platform_id, &record.conversation_id);
        self.conversations
            .write()
            .map_err(lock_error)?
            .insert(key, record);
        Ok(())
    }

    async fn conversation(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ConversationRecord>> {
        Ok(self
            .conversations
            .read()
            .map_err(lock_error)?
            .get(&ConversationKey::new(platform_id, conversation_id))
            .cloned())
    }

    async fn delete_conversation(&self, platform_id: &str, conversation_id: &str) -> Result<bool> {
        let key = ConversationKey::new(platform_id, conversation_id);
        let removed = self
            .conversations
            .write()
            .map_err(lock_error)?
            .remove(&key)
            .is_some();
        if removed {
            self.current_by_platform
                .write()
                .map_err(lock_error)?
                .retain(|_, current_key| current_key != &key);
        }
        Ok(removed)
    }

    async fn set_current_conversation(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<()> {
        let key = ConversationKey::new(platform_id, conversation_id);
        if !self
            .conversations
            .read()
            .map_err(lock_error)?
            .contains_key(&key)
        {
            return Err(AstrbotError::Pipeline(format!(
                "conversation {platform_id}:{conversation_id} is not registered"
            )));
        }
        self.current_by_platform
            .write()
            .map_err(lock_error)?
            .insert(platform_id.to_string(), key);
        Ok(())
    }

    async fn current_conversation(&self, platform_id: &str) -> Result<Option<ConversationRecord>> {
        let current_key = self
            .current_by_platform
            .read()
            .map_err(lock_error)?
            .get(platform_id)
            .cloned();
        let Some(current_key) = current_key else {
            return Ok(None);
        };
        Ok(self
            .conversations
            .read()
            .map_err(lock_error)?
            .get(&current_key)
            .cloned())
    }
}

#[derive(Clone)]
pub struct ConversationService {
    directory: Arc<dyn ConversationDirectory>,
}

impl Default for ConversationService {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationService {
    pub fn new() -> Self {
        Self::with_directory(Arc::new(InMemoryConversationDirectory::new()))
    }

    pub fn with_directory(directory: Arc<dyn ConversationDirectory>) -> Self {
        Self { directory }
    }

    pub async fn upsert(&self, record: ConversationRecord) -> Result<()> {
        self.directory.upsert_conversation(record).await
    }

    pub async fn get(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ConversationRecord>> {
        self.directory
            .conversation(platform_id, conversation_id)
            .await
    }

    pub async fn delete(&self, platform_id: &str, conversation_id: &str) -> Result<bool> {
        self.directory
            .delete_conversation(platform_id, conversation_id)
            .await
    }

    pub async fn switch_current(&self, platform_id: &str, conversation_id: &str) -> Result<()> {
        self.directory
            .set_current_conversation(platform_id, conversation_id)
            .await
    }

    pub async fn current(&self, platform_id: &str) -> Result<Option<ConversationRecord>> {
        self.directory.current_conversation(platform_id).await
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ConversationKey {
    platform_id: String,
    conversation_id: String,
}

impl ConversationKey {
    fn new(platform_id: &str, conversation_id: &str) -> Self {
        Self {
            platform_id: platform_id.to_string(),
            conversation_id: conversation_id.to_string(),
        }
    }
}

fn lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("conversation directory lock: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{ConversationRecord, ConversationService};

    #[tokio::test]
    async fn conversation_service_tracks_current_conversation_per_platform() {
        let service = ConversationService::new();
        service
            .upsert(
                ConversationRecord::new("webchat", "conversation-1")
                    .with_title("General")
                    .with_persona_id("persona-a"),
            )
            .await
            .expect("conversation should upsert");

        service
            .switch_current("webchat", "conversation-1")
            .await
            .expect("conversation should become current");

        let current = service
            .current("webchat")
            .await
            .expect("current conversation should load")
            .expect("current conversation should exist");
        assert_eq!(current.conversation_id, "conversation-1");
        assert_eq!(current.persona_id.as_deref(), Some("persona-a"));
    }

    #[tokio::test]
    async fn deleting_current_conversation_clears_current_pointer() {
        let service = ConversationService::new();
        service
            .upsert(ConversationRecord::new("webchat", "conversation-1"))
            .await
            .expect("conversation should upsert");
        service
            .switch_current("webchat", "conversation-1")
            .await
            .expect("conversation should become current");

        assert!(
            service
                .delete("webchat", "conversation-1")
                .await
                .expect("conversation should delete")
        );
        assert!(
            service
                .current("webchat")
                .await
                .expect("current conversation should load")
                .is_none()
        );
    }
}
