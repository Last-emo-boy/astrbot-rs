use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_core::{AstrbotError, Result};
use astrbot_storage::SqliteJsonStore;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationRecord {
    pub platform_id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<String>,
    pub title: Option<String>,
    pub persona_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ConversationCurrentPointer {
    platform_id: String,
    conversation_id: String,
}

impl ConversationRecord {
    pub fn new(platform_id: impl Into<String>, conversation_id: impl Into<String>) -> Self {
        let now = unix_timestamp();
        Self {
            platform_id: platform_id.into(),
            conversation_id: conversation_id.into(),
            user_id: None,
            history: None,
            title: None,
            persona_id: None,
            created_at: Some(now),
            updated_at: Some(now),
            token_usage: Some(0),
        }
    }

    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    pub fn with_history(mut self, history: impl Into<String>) -> Self {
        self.history = Some(history.into());
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_persona_id(mut self, persona_id: impl Into<String>) -> Self {
        self.persona_id = Some(persona_id.into());
        self
    }

    pub fn with_created_at(mut self, created_at: i64) -> Self {
        self.created_at = Some(created_at);
        self
    }

    pub fn with_updated_at(mut self, updated_at: i64) -> Self {
        self.updated_at = Some(updated_at);
        self
    }

    pub fn with_token_usage(mut self, token_usage: u64) -> Self {
        self.token_usage = Some(token_usage);
        self
    }

    pub fn touch(mut self) -> Self {
        self.updated_at = Some(unix_timestamp());
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

    async fn list_conversations(
        &self,
        platform_id: Option<&str>,
    ) -> Result<Vec<ConversationRecord>>;

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

#[derive(Clone, Debug)]
pub struct SqliteConversationDirectory {
    store: SqliteJsonStore,
}

impl SqliteConversationDirectory {
    pub fn new(store: SqliteJsonStore) -> Self {
        Self { store }
    }
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

    async fn list_conversations(
        &self,
        platform_id: Option<&str>,
    ) -> Result<Vec<ConversationRecord>> {
        let mut records = self
            .conversations
            .read()
            .map_err(lock_error)?
            .values()
            .filter(|record| {
                platform_id
                    .map(|platform_id| record.platform_id == platform_id)
                    .unwrap_or(true)
            })
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.platform_id
                .cmp(&right.platform_id)
                .then_with(|| left.conversation_id.cmp(&right.conversation_id))
        });
        Ok(records)
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

#[async_trait]
impl ConversationDirectory for SqliteConversationDirectory {
    async fn upsert_conversation(&self, record: ConversationRecord) -> Result<()> {
        let key = conversation_key(&record.platform_id, &record.conversation_id);
        self.store.put_json("conversation_directory", &key, &record)
    }

    async fn conversation(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ConversationRecord>> {
        self.store.get_json(
            "conversation_directory",
            &conversation_key(platform_id, conversation_id),
        )
    }

    async fn list_conversations(
        &self,
        platform_id: Option<&str>,
    ) -> Result<Vec<ConversationRecord>> {
        let mut records = self
            .store
            .list_json::<ConversationRecord>("conversation_directory")?
            .into_iter()
            .filter(|record| {
                platform_id
                    .map(|platform_id| record.platform_id == platform_id)
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.platform_id
                .cmp(&right.platform_id)
                .then_with(|| left.conversation_id.cmp(&right.conversation_id))
        });
        Ok(records)
    }

    async fn delete_conversation(&self, platform_id: &str, conversation_id: &str) -> Result<bool> {
        let removed = self.store.delete_json(
            "conversation_directory",
            &conversation_key(platform_id, conversation_id),
        )?;
        if removed
            && self
                .current_conversation(platform_id)
                .await?
                .is_some_and(|record| record.conversation_id == conversation_id)
        {
            self.store
                .delete_json("conversation_current", platform_id)?;
        }
        Ok(removed)
    }

    async fn set_current_conversation(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<()> {
        if self
            .conversation(platform_id, conversation_id)
            .await?
            .is_none()
        {
            return Err(AstrbotError::Pipeline(format!(
                "conversation {platform_id}:{conversation_id} is not registered"
            )));
        }
        self.store.put_json(
            "conversation_current",
            platform_id,
            &ConversationCurrentPointer {
                platform_id: platform_id.to_string(),
                conversation_id: conversation_id.to_string(),
            },
        )
    }

    async fn current_conversation(&self, platform_id: &str) -> Result<Option<ConversationRecord>> {
        let Some(pointer) = self
            .store
            .get_json::<ConversationCurrentPointer>("conversation_current", platform_id)?
        else {
            return Ok(None);
        };
        self.conversation(&pointer.platform_id, &pointer.conversation_id)
            .await
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

    pub async fn list(&self, platform_id: Option<&str>) -> Result<Vec<ConversationRecord>> {
        self.directory.list_conversations(platform_id).await
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

fn conversation_key(platform_id: &str, conversation_id: &str) -> String {
    format!("{platform_id}\u{1f}{conversation_id}")
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use astrbot_storage::SqliteJsonStore;

    use super::{ConversationRecord, ConversationService, SqliteConversationDirectory};

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

    #[tokio::test]
    async fn conversation_service_lists_by_platform_in_stable_order() {
        let service = ConversationService::new();
        service
            .upsert(ConversationRecord::new("webchat", "conversation-b"))
            .await
            .expect("conversation should upsert");
        service
            .upsert(ConversationRecord::new("console", "conversation-a"))
            .await
            .expect("conversation should upsert");
        service
            .upsert(ConversationRecord::new("webchat", "conversation-a"))
            .await
            .expect("conversation should upsert");

        let all = service.list(None).await.expect("conversations should list");
        assert_eq!(
            all.iter()
                .map(|record| format!("{}:{}", record.platform_id, record.conversation_id))
                .collect::<Vec<_>>(),
            vec![
                "console:conversation-a".to_string(),
                "webchat:conversation-a".to_string(),
                "webchat:conversation-b".to_string()
            ]
        );

        let webchat = service
            .list(Some("webchat"))
            .await
            .expect("webchat conversations should list");
        assert_eq!(webchat.len(), 2);
        assert!(webchat.iter().all(|record| record.platform_id == "webchat"));
    }

    #[tokio::test]
    async fn sqlite_conversation_directory_tracks_current_across_services() {
        let store = SqliteJsonStore::open_in_memory().expect("sqlite store should open");
        let service = ConversationService::with_directory(Arc::new(
            SqliteConversationDirectory::new(store.clone()),
        ));
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

        let reloaded =
            ConversationService::with_directory(Arc::new(SqliteConversationDirectory::new(store)));
        assert_eq!(
            reloaded
                .current("webchat")
                .await
                .expect("current conversation should load")
                .expect("current conversation should exist")
                .persona_id
                .as_deref(),
            Some("persona-a")
        );
    }
}
