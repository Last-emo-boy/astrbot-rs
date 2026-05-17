use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationReferenceRecord {
    pub conversation_id: String,
    pub message_id: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub refs: Value,
}

impl ConversationReferenceRecord {
    pub fn new(
        conversation_id: impl Into<String>,
        message_id: impl Into<String>,
        refs: impl Into<Value>,
    ) -> Self {
        Self {
            conversation_id: conversation_id.into(),
            message_id: message_id.into(),
            refs: refs.into(),
        }
    }
}

#[async_trait]
pub trait ConversationReferenceRepository: Send + Sync {
    async fn save_references(&self, record: ConversationReferenceRecord) -> Result<()>;

    async fn references_for_message(
        &self,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<Option<ConversationReferenceRecord>>;

    async fn references_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationReferenceRecord>>;
}

#[derive(Clone, Default)]
pub struct InMemoryConversationReferenceRepository {
    records: Arc<RwLock<BTreeMap<(String, String), ConversationReferenceRecord>>>,
}

impl InMemoryConversationReferenceRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ConversationReferenceRepository for InMemoryConversationReferenceRepository {
    async fn save_references(&self, record: ConversationReferenceRecord) -> Result<()> {
        self.records
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("conversation refs lock: {err}")))?
            .insert(
                (record.conversation_id.clone(), record.message_id.clone()),
                record,
            );
        Ok(())
    }

    async fn references_for_message(
        &self,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<Option<ConversationReferenceRecord>> {
        Ok(self
            .records
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("conversation refs lock: {err}")))?
            .get(&(conversation_id.to_string(), message_id.to_string()))
            .cloned())
    }

    async fn references_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationReferenceRecord>> {
        Ok(self
            .records
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("conversation refs lock: {err}")))?
            .values()
            .filter(|record| record.conversation_id == conversation_id)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        ConversationReferenceRecord, ConversationReferenceRepository,
        InMemoryConversationReferenceRepository,
    };

    #[tokio::test]
    async fn in_memory_conversation_refs_are_scoped_by_conversation_and_message() {
        let repository = InMemoryConversationReferenceRepository::new();
        repository
            .save_references(ConversationReferenceRecord::new(
                "conversation-a",
                "message-1",
                json!({"used": [{"index": "abcd.1"}]}),
            ))
            .await
            .expect("refs should save");
        repository
            .save_references(ConversationReferenceRecord::new(
                "conversation-b",
                "message-2",
                json!({"used": [{"index": "other.1"}]}),
            ))
            .await
            .expect("refs should save");

        let refs = repository
            .references_for_message("conversation-a", "message-1")
            .await
            .expect("refs should load")
            .expect("message refs should exist");
        assert_eq!(refs.refs["used"][0]["index"], "abcd.1");

        let conversation_refs = repository
            .references_for_conversation("conversation-a")
            .await
            .expect("conversation refs should load");
        assert_eq!(conversation_refs.len(), 1);
        assert_eq!(conversation_refs[0].message_id, "message-1");
    }
}
