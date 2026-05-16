use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{MessageEvent, Result};

#[async_trait]
pub trait EventExecutor: Send + Sync {
    async fn execute(&self, event: MessageEvent) -> Result<()>;
}

pub struct EventBus {
    receiver: mpsc::Receiver<MessageEvent>,
    executor: Arc<dyn EventExecutor>,
}

impl EventBus {
    pub fn new(receiver: mpsc::Receiver<MessageEvent>, executor: Arc<dyn EventExecutor>) -> Self {
        Self { receiver, executor }
    }

    pub async fn run_once(&mut self) -> Result<bool> {
        let Some(event) = self.receiver.recv().await else {
            return Ok(false);
        };

        self.executor.execute(event).await?;
        Ok(true)
    }

    pub async fn run(mut self) -> Result<()> {
        while self.run_once().await? {}
        Ok(())
    }
}
