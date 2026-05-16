use async_trait::async_trait;

use crate::Result;

use super::chain::MessageChain;
use super::result::MessageStream;
use super::session::MessageSession;

#[async_trait]
pub trait MessageSink: Send + Sync {
    async fn send(&self, session: &MessageSession, chain: MessageChain) -> Result<()>;

    async fn send_streaming(&self, session: &MessageSession, stream: MessageStream) -> Result<()> {
        for chain in stream.into_chunks() {
            self.send(session, chain).await?;
        }
        Ok(())
    }
}
