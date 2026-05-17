use astrbot_conversation::{ConversationMessageRecord, PlatformMessageHistoryService};
use astrbot_core::{MessageChain, MessageSession, MessageSink, MessageStream, Result};
use async_trait::async_trait;
use tokio::sync::Mutex;

use super::MessageRecorder;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentMessage {
    pub session: MessageSession,
    pub chain: MessageChain,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamedMessage {
    pub session: MessageSession,
    pub stream: MessageStream,
}

#[derive(Default)]
pub struct RecordingSink {
    sent: Mutex<Vec<SentMessage>>,
    streamed: Mutex<Vec<StreamedMessage>>,
}

impl RecordingSink {
    pub async fn messages(&self) -> Vec<SentMessage> {
        self.sent.lock().await.clone()
    }

    pub async fn streaming_messages(&self) -> Vec<StreamedMessage> {
        self.streamed.lock().await.clone()
    }
}

#[async_trait]
impl MessageRecorder for RecordingSink {
    async fn messages(&self) -> Vec<SentMessage> {
        self.sent.lock().await.clone()
    }

    async fn streaming_messages(&self) -> Vec<StreamedMessage> {
        self.streamed.lock().await.clone()
    }
}

#[async_trait]
impl PlatformMessageHistoryService for RecordingSink {
    async fn append_message(&self, record: ConversationMessageRecord) -> Result<()> {
        self.send(&record.session, record.chain).await
    }

    async fn messages_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationMessageRecord>> {
        Ok(self
            .messages()
            .await
            .into_iter()
            .filter(|sent| sent.session.conversation_id == conversation_id)
            .map(|sent| ConversationMessageRecord::new(sent.session, sent.chain))
            .collect())
    }
}

#[async_trait]
impl MessageSink for RecordingSink {
    async fn send(&self, session: &MessageSession, chain: MessageChain) -> Result<()> {
        self.sent.lock().await.push(SentMessage {
            session: session.clone(),
            chain,
        });
        Ok(())
    }

    async fn send_streaming(&self, session: &MessageSession, stream: MessageStream) -> Result<()> {
        self.streamed.lock().await.push(StreamedMessage {
            session: session.clone(),
            stream,
        });
        Ok(())
    }
}
