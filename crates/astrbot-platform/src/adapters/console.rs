use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use astrbot_core::{
    AstrbotError, MessageChain, MessageEvent, MessageSender, MessageSession, MessageSink, Result,
};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{Mutex, mpsc};

use crate::{MessageRecorder, PlatformAdapter, PlatformIdentityNormalizer, SentMessage};
#[derive(Default)]
pub struct ConsoleSink {
    sent: Mutex<Vec<SentMessage>>,
}

impl ConsoleSink {
    pub async fn messages(&self) -> Vec<SentMessage> {
        self.sent.lock().await.clone()
    }
}

#[async_trait]
impl MessageRecorder for ConsoleSink {
    async fn messages(&self) -> Vec<SentMessage> {
        self.sent.lock().await.clone()
    }
}

#[async_trait]
impl MessageSink for ConsoleSink {
    async fn send(&self, session: &MessageSession, chain: MessageChain) -> Result<()> {
        let mut stdout = std::io::stdout();
        writeln!(
            stdout,
            "[{}] {}",
            session.conversation_id,
            chain.plain_text()
        )
        .map_err(|err| AstrbotError::Platform(format!("console output failed: {err}")))?;
        self.sent.lock().await.push(SentMessage {
            session: session.clone(),
            chain,
        });
        Ok(())
    }
}

pub struct ConsolePlatform {
    id: String,
    name: String,
    event_sender: mpsc::Sender<MessageEvent>,
    sink: Arc<dyn MessageSink>,
    event_counter: AtomicU64,
}

impl ConsolePlatform {
    pub fn new(event_sender: mpsc::Sender<MessageEvent>, sink: Arc<dyn MessageSink>) -> Self {
        Self::with_identity("console", "Console Platform", event_sender, sink)
    }

    pub fn with_identity(
        id: impl Into<String>,
        name: impl Into<String>,
        event_sender: mpsc::Sender<MessageEvent>,
        sink: Arc<dyn MessageSink>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            event_sender,
            sink,
            event_counter: AtomicU64::new(1),
        }
    }

    pub async fn handle_line(&self, line: impl Into<String>) -> Result<bool> {
        let line = line.into();
        let line = line.trim();
        if line.is_empty() {
            return Ok(false);
        }

        let (sender_id, text) = parse_console_line(line);
        let event_id = format!(
            "{}-event-{}",
            self.id,
            self.event_counter.fetch_add(1, Ordering::Relaxed)
        );
        let sender = MessageSender::new(sender_id, None);
        let identity = PlatformIdentityNormalizer::normalize_direct_event(&sender);
        let event = MessageEvent::new(
            event_id,
            self.id.clone(),
            self.name.clone(),
            MessageSession::new(self.id.clone(), "console"),
            sender,
            MessageChain::plain(text),
            self.sink.clone(),
        )
        .with_identity(identity);

        self.event_sender
            .send(event)
            .await
            .map_err(|_| AstrbotError::EventChannelClosed)?;
        Ok(true)
    }
}

#[async_trait]
impl PlatformAdapter for ConsolePlatform {
    async fn run(&self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut lines = BufReader::new(stdin).lines();

        while let Some(line) = lines
            .next_line()
            .await
            .map_err(|err| AstrbotError::Platform(format!("console input failed: {err}")))?
        {
            self.handle_line(line).await?;
        }

        Ok(())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn parse_console_line(line: &str) -> (String, String) {
    if let Some((sender_id, text)) = line.split_once(':') {
        let sender_id = sender_id.trim();
        let text = text.trim();
        if !sender_id.is_empty() && !text.is_empty() {
            return (sender_id.to_string(), text.to_string());
        }
    }

    ("console-user".to_string(), line.to_string())
}
