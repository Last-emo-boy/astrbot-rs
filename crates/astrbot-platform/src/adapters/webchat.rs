use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use astrbot_core::{
    AstrbotError, MessageChain, MessageComponent, MessageEvent, MessageSender, MessageSession,
    Result,
};
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{PlatformAdapter, RecordingSink, SentMessage};
pub struct WebChatPlatform {
    id: String,
    name: String,
    event_sender: mpsc::Sender<MessageEvent>,
    sink: Arc<RecordingSink>,
    event_counter: AtomicU64,
}

impl WebChatPlatform {
    pub fn new(event_sender: mpsc::Sender<MessageEvent>, sink: Arc<RecordingSink>) -> Self {
        Self::with_identity("webchat", "WebChat Platform", event_sender, sink)
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
            event_counter: AtomicU64::new(1),
        }
    }

    pub async fn submit_text(
        &self,
        conversation_id: impl Into<String>,
        sender_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<String> {
        self.submit_message(conversation_id, sender_id, text, Vec::new())
            .await
    }

    pub async fn submit_message(
        &self,
        conversation_id: impl Into<String>,
        sender_id: impl Into<String>,
        text: impl Into<String>,
        image_urls: Vec<String>,
    ) -> Result<String> {
        let text = text.into();
        let mut message = MessageChain::default();
        if !text.trim().is_empty() {
            message.push(MessageComponent::plain(text));
        }
        for image_url in image_urls {
            let image_url = image_url.trim();
            if !image_url.is_empty() {
                message.push(MessageComponent::image(image_url.to_string()));
            }
        }

        self.submit_chain(conversation_id, sender_id, message).await
    }

    pub async fn submit_chain(
        &self,
        conversation_id: impl Into<String>,
        sender_id: impl Into<String>,
        message: MessageChain,
    ) -> Result<String> {
        if message.is_empty() {
            return Err(AstrbotError::EmptyMessage);
        }

        let event_id = format!(
            "{}-event-{}",
            self.id,
            self.event_counter.fetch_add(1, Ordering::Relaxed)
        );
        let event = MessageEvent::new(
            event_id.clone(),
            self.id.clone(),
            self.name.clone(),
            MessageSession::new(self.id.clone(), conversation_id),
            MessageSender::new(sender_id, None),
            message,
            self.sink.clone(),
        );

        self.event_sender
            .send(event)
            .await
            .map_err(|_| AstrbotError::EventChannelClosed)?;
        Ok(event_id)
    }

    pub fn sink(&self) -> Arc<RecordingSink> {
        self.sink.clone()
    }

    pub async fn sent_messages(&self) -> Vec<SentMessage> {
        self.sink.messages().await
    }

    pub async fn sent_messages_for_conversation(&self, conversation_id: &str) -> Vec<SentMessage> {
        self.sent_messages()
            .await
            .into_iter()
            .filter(|sent| sent.session.conversation_id == conversation_id)
            .collect()
    }
}

#[async_trait]
impl PlatformAdapter for WebChatPlatform {
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
