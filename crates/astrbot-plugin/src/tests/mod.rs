use std::sync::Arc;

use astrbot_core::{
    MessageChain, MessageEvent, MessageSender, MessageSession, MessageSink, Result,
};
use async_trait::async_trait;

struct NoopSink;

#[async_trait]
impl MessageSink for NoopSink {
    async fn send(&self, _session: &MessageSession, _chain: MessageChain) -> Result<()> {
        Ok(())
    }
}

fn event(message: impl Into<String>) -> MessageEvent {
    MessageEvent::new(
        "event",
        "console",
        "console",
        MessageSession::new("console", "user"),
        MessageSender::new("user", None),
        MessageChain::plain(message),
        Arc::new(NoopSink),
    )
}

mod dependency;
mod filters;
mod loader;
mod manifest_sdk;
mod market;
mod registry;
mod sandbox;
mod tool;
