use std::sync::Arc;

use astrbot_core::{
    AstrbotError, MessageChain, MessageEvent, MessageSender, MessageSession, Result,
};
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{PlatformAdapter, PlatformIdentityNormalizer, RecordingSink};
pub struct MockPlatform {
    id: String,
    name: String,
    event_sender: mpsc::Sender<MessageEvent>,
    sink: Arc<RecordingSink>,
}

impl MockPlatform {
    pub fn new(event_sender: mpsc::Sender<MessageEvent>, sink: Arc<RecordingSink>) -> Self {
        Self::with_identity("mock", "Mock Platform", event_sender, sink)
    }

    pub fn with_identity(
        id: impl Into<String>,
        name: impl Into<String>,
        event_sender: mpsc::Sender<MessageEvent>,
        sink: Arc<RecordingSink>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            event_sender,
            sink,
        }
    }

    pub async fn emit_text(
        &self,
        event_id: impl Into<String>,
        conversation_id: impl Into<String>,
        sender_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<()> {
        let sender = MessageSender::new(sender_id, None);
        let identity = PlatformIdentityNormalizer::normalize_direct_event(&sender);
        let event = MessageEvent::new(
            event_id,
            self.id.clone(),
            self.name.clone(),
            MessageSession::new(self.id.clone(), conversation_id),
            sender,
            MessageChain::plain(text),
            self.sink.clone(),
        )
        .with_identity(identity);

        self.event_sender
            .send(event)
            .await
            .map_err(|_| AstrbotError::EventChannelClosed)
    }

    pub fn sink(&self) -> Arc<RecordingSink> {
        self.sink.clone()
    }
}

#[async_trait]
impl PlatformAdapter for MockPlatform {
    async fn run(&self) -> Result<()> {
        Ok(())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }
}
