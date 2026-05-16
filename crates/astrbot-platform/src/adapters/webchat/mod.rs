use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use astrbot_conversation::PlatformMessageHistoryService;
use astrbot_core::{AstrbotError, MessageChain, MessageEvent, Result};
use async_trait::async_trait;
use tokio::sync::mpsc;

use self::event::build_webchat_event;
use self::message::message_chain_from_text_and_images;
use crate::{PlatformAdapter, RecordingSink, SentMessage};

mod event;
mod message;

pub struct WebChatPlatform {
    id: String,
    name: String,
    event_sender: mpsc::Sender<MessageEvent>,
    sink: Arc<RecordingSink>,
    history: Arc<dyn PlatformMessageHistoryService>,
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
            history: sink.clone(),
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
        let message = message_chain_from_text_and_images(text, image_urls);
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
        let event = build_webchat_event(
            event_id.clone(),
            self.id.clone(),
            self.name.clone(),
            conversation_id,
            sender_id,
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

    pub fn conversation_history(&self) -> Arc<dyn PlatformMessageHistoryService> {
        self.history.clone()
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
