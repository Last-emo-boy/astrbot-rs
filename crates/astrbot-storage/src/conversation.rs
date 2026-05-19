use std::sync::RwLock;

use astrbot_core::{AstrbotError, MessageChain, MessageSession, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::SqliteJsonStore;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationMessageRecord {
    pub message_id: Option<String>,
    pub session: MessageSession,
    pub chain: MessageChain,
}

#[derive(Clone, Debug)]
pub struct SqliteConversationHistoryRepository {
    store: SqliteJsonStore,
}

impl SqliteConversationHistoryRepository {
    pub fn new(store: SqliteJsonStore) -> Self {
        Self { store }
    }
}

impl ConversationMessageRecord {
    pub fn new(session: MessageSession, chain: MessageChain) -> Self {
        Self {
            message_id: None,
            session,
            chain,
        }
    }

    pub fn with_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.message_id = Some(message_id.into());
        self
    }
}

#[async_trait]
pub trait ConversationHistoryRepository: Send + Sync {
    async fn append_message(&self, record: ConversationMessageRecord) -> Result<()>;

    async fn messages_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationMessageRecord>>;
}

#[derive(Default)]
pub struct InMemoryConversationHistoryRepository {
    messages: RwLock<Vec<ConversationMessageRecord>>,
}

impl InMemoryConversationHistoryRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ConversationHistoryRepository for InMemoryConversationHistoryRepository {
    async fn append_message(&self, record: ConversationMessageRecord) -> Result<()> {
        self.messages
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("conversation history lock: {err}")))?
            .push(record);
        Ok(())
    }

    async fn messages_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationMessageRecord>> {
        Ok(self
            .messages
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("conversation history lock: {err}")))?
            .iter()
            .filter(|record| record.session.conversation_id == conversation_id)
            .cloned()
            .collect())
    }
}

#[async_trait]
impl ConversationHistoryRepository for SqliteConversationHistoryRepository {
    async fn append_message(&self, record: ConversationMessageRecord) -> Result<()> {
        let key = match record
            .message_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            Some(message_id) => message_id.to_string(),
            None => format!(
                "message-{:020}",
                self.store.next_record_id("conversation_messages")?
            ),
        };
        self.store.put_json("conversation_messages", &key, &record)
    }

    async fn messages_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationMessageRecord>> {
        Ok(self
            .store
            .list_json::<ConversationMessageRecord>("conversation_messages")?
            .into_iter()
            .filter(|record| record.session.conversation_id == conversation_id)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ConversationHistoryRepository, ConversationMessageRecord,
        InMemoryConversationHistoryRepository, SqliteConversationHistoryRepository,
    };
    use crate::SqliteJsonStore;
    use astrbot_core::{MessageChain, MessageSession};

    #[tokio::test]
    async fn in_memory_conversation_history_filters_by_conversation() {
        let repository = InMemoryConversationHistoryRepository::new();
        repository
            .append_message(ConversationMessageRecord::new(
                MessageSession::new("webchat", "conversation-a"),
                MessageChain::plain("alpha"),
            ))
            .await
            .expect("message should append");
        repository
            .append_message(ConversationMessageRecord::new(
                MessageSession::new("webchat", "conversation-b"),
                MessageChain::plain("beta"),
            ))
            .await
            .expect("message should append");

        let messages = repository
            .messages_for_conversation("conversation-a")
            .await
            .expect("history should load");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].chain.plain_text(), "alpha");
    }

    #[tokio::test]
    async fn sqlite_conversation_history_filters_by_conversation() {
        let repository = SqliteConversationHistoryRepository::new(
            SqliteJsonStore::open_in_memory().expect("sqlite store should open"),
        );
        repository
            .append_message(ConversationMessageRecord::new(
                MessageSession::new("webchat", "conversation-a"),
                MessageChain::plain("alpha"),
            ))
            .await
            .expect("message should append");
        repository
            .append_message(ConversationMessageRecord::new(
                MessageSession::new("webchat", "conversation-b"),
                MessageChain::plain("beta"),
            ))
            .await
            .expect("message should append");

        let messages = repository
            .messages_for_conversation("conversation-a")
            .await
            .expect("history should load");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].chain.plain_text(), "alpha");
    }
}
