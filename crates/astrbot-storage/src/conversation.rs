use std::sync::RwLock;

use astrbot_core::{AstrbotError, MessageChain, MessageSession, Result};
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationMessageRecord {
    pub message_id: Option<String>,
    pub session: MessageSession,
    pub chain: MessageChain,
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

#[cfg(test)]
mod tests {
    use super::{
        ConversationHistoryRepository, ConversationMessageRecord,
        InMemoryConversationHistoryRepository,
    };
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
}
