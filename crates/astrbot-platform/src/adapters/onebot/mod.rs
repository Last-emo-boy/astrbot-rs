use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use astrbot_core::{AstrbotError, MessageChain, MessageEvent, Result};
use async_trait::async_trait;
use tokio::sync::mpsc;

use self::event::build_onebot_event;
use self::message::plain_text_message;
use crate::{PlatformAdapter, PlatformTransport, RecordingSink, SentMessage};

mod event;
mod forward;
mod message;
mod session;
mod transport;

pub use forward::{OneBotForwardParseResult, OneBotForwardParser};
pub use session::{OneBotSession, OneBotSessionKind};
pub use transport::{OneBotTransport, OneBotTransportMode};

pub struct OneBotPlatform {
    id: String,
    name: String,
    event_sender: mpsc::Sender<MessageEvent>,
    sink: Arc<RecordingSink>,
    transport: OneBotTransport,
    event_counter: AtomicU64,
}

impl OneBotPlatform {
    pub fn new(event_sender: mpsc::Sender<MessageEvent>, sink: Arc<RecordingSink>) -> Self {
        Self::with_identity("onebot", "OneBot Platform", event_sender, sink)
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
            transport: OneBotTransport::in_process(),
            event_counter: AtomicU64::new(1),
        }
    }

    pub fn with_transport(mut self, transport: OneBotTransport) -> Self {
        self.transport = transport;
        self
    }

    pub async fn submit_private_text(
        &self,
        user_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<String> {
        let user_id = user_id.into();
        self.submit_private_chain(user_id, plain_text_message(text))
            .await
    }

    pub async fn submit_group_text(
        &self,
        group_id: impl Into<String>,
        sender_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<String> {
        self.submit_group_chain(group_id, sender_id, plain_text_message(text))
            .await
    }

    pub async fn submit_private_chain(
        &self,
        user_id: impl Into<String>,
        message: MessageChain,
    ) -> Result<String> {
        let user_id = user_id.into();
        self.submit_chain(OneBotSession::private(&self.id, &user_id), user_id, message)
            .await
    }

    pub async fn submit_group_chain(
        &self,
        group_id: impl Into<String>,
        sender_id: impl Into<String>,
        message: MessageChain,
    ) -> Result<String> {
        let group_id = group_id.into();
        self.submit_chain(OneBotSession::group(&self.id, group_id), sender_id, message)
            .await
    }

    async fn submit_chain(
        &self,
        session: OneBotSession,
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
        let event = build_onebot_event(
            event_id.clone(),
            self.id.clone(),
            self.name.clone(),
            session.message_session(),
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

    pub async fn sent_messages_for_conversation(&self, conversation_id: &str) -> Vec<SentMessage> {
        self.sink
            .messages()
            .await
            .into_iter()
            .filter(|sent| sent.session.conversation_id == conversation_id)
            .collect()
    }
}

#[async_trait]
impl PlatformAdapter for OneBotPlatform {
    async fn run(&self) -> Result<()> {
        self.transport.run().await
    }

    async fn terminate(&self) -> Result<()> {
        self.transport.terminate().await
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }
}
