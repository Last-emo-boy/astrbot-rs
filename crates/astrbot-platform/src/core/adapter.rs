use astrbot_core::Result;
use async_trait::async_trait;

use super::{SentMessage, StreamedMessage};

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
