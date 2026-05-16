use std::sync::Arc;

use astrbot_core::Result;
use astrbot_storage::ConversationHistoryRepository;
pub use astrbot_storage::ConversationMessageRecord;
use async_trait::async_trait;

#[async_trait]
pub trait PlatformMessageHistoryService: Send + Sync {
    async fn append_message(&self, record: ConversationMessageRecord) -> Result<()>;

    async fn messages_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationMessageRecord>>;
}

pub struct RepositoryMessageHistoryService {
    repository: Arc<dyn ConversationHistoryRepository>,
}

impl RepositoryMessageHistoryService {
    pub fn new(repository: Arc<dyn ConversationHistoryRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl PlatformMessageHistoryService for RepositoryMessageHistoryService {
    async fn append_message(&self, record: ConversationMessageRecord) -> Result<()> {
        self.repository.append_message(record).await
    }

    async fn messages_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationMessageRecord>> {
        self.repository
            .messages_for_conversation(conversation_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use astrbot_core::{MessageChain, MessageSession};
    use astrbot_storage::InMemoryConversationHistoryRepository;

    use super::{
        ConversationMessageRecord, PlatformMessageHistoryService, RepositoryMessageHistoryService,
    };

    #[tokio::test]
    async fn repository_message_history_service_delegates_storage_history() {
        let service = RepositoryMessageHistoryService::new(Arc::new(
            InMemoryConversationHistoryRepository::new(),
        ));
        service
            .append_message(ConversationMessageRecord::new(
                MessageSession::new("webchat", "conversation-a"),
                MessageChain::plain("hello"),
            ))
            .await
            .expect("message should append");
        service
            .append_message(ConversationMessageRecord::new(
                MessageSession::new("webchat", "conversation-b"),
                MessageChain::plain("other"),
            ))
            .await
            .expect("message should append");

        let messages = service
            .messages_for_conversation("conversation-a")
            .await
            .expect("history should load");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].chain.plain_text(), "hello");
    }
}
