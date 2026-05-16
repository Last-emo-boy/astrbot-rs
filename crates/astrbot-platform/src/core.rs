use astrbot_conversation::{ConversationMessageRecord, PlatformMessageHistoryService};
use astrbot_core::{
    AstrbotError, MessageChain, MessageEvent, MessageSession, MessageSink, MessageStream, Result,
};
use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
pub const CONSOLE_PLATFORM_TYPE: &str = "console";
pub const WEBCHAT_PLATFORM_TYPE: &str = "webchat";
pub const ONEBOT_PLATFORM_TYPE: &str = "onebot";
pub const MOCK_PLATFORM_TYPE: &str = "mock";

#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    async fn run(&self) -> Result<()>;

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }

    fn id(&self) -> &str;
    fn name(&self) -> &str;
}

#[async_trait]
pub trait MessageRecorder: Send + Sync {
    async fn messages(&self) -> Vec<SentMessage>;

    async fn streaming_messages(&self) -> Vec<StreamedMessage> {
        Vec::new()
    }
}

pub struct PlatformConfig {
    pub id: String,
    pub platform_type: String,
    pub enabled: bool,
    pub name: Option<String>,
}

impl PlatformConfig {
    pub fn mock(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: MOCK_PLATFORM_TYPE.to_string(),
            enabled: true,
            name: None,
        }
    }

    pub fn console(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: CONSOLE_PLATFORM_TYPE.to_string(),
            enabled: true,
            name: None,
        }
    }

    pub fn webchat(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: WEBCHAT_PLATFORM_TYPE.to_string(),
            enabled: true,
            name: None,
        }
    }

    pub fn onebot(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: ONEBOT_PLATFORM_TYPE.to_string(),
            enabled: true,
            name: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

#[derive(Clone)]
pub struct PlatformBuildContext {
    event_sender: mpsc::Sender<MessageEvent>,
}

impl PlatformBuildContext {
    pub fn new(event_sender: mpsc::Sender<MessageEvent>) -> Self {
        Self { event_sender }
    }

    pub fn event_sender(&self) -> mpsc::Sender<MessageEvent> {
        self.event_sender.clone()
    }
}

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

pub(crate) fn validate_platform_id(id: &str) -> Result<()> {
    if id.trim().is_empty() {
        return Err(AstrbotError::Platform(
            "platform id must not be empty".to_string(),
        ));
    }
    if id.contains(':') || id.contains('!') {
        return Err(AstrbotError::Platform(format!(
            "platform id {id} must not contain ':' or '!'"
        )));
    }
    Ok(())
}
