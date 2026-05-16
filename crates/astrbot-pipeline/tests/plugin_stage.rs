use std::sync::Arc;

use astrbot_core::{EventBus, MessageEvent, MessageEventResult, Result};
use astrbot_pipeline::{
    PipelineContext, PipelineScheduler,
    stages::{PluginStage, ProviderStage, RespondStage},
};
use astrbot_platform::{MockPlatform, RecordingSink};
use astrbot_plugin::{
    CommandFilter, HandlerMetadata, PluginControl, PluginEventType, PluginHandler, PluginRegistry,
    RegisteredHandler,
};
use astrbot_provider::MockChatProvider;
use async_trait::async_trait;
use tokio::sync::mpsc;

struct StaticReplyHandler {
    reply: &'static str,
}

#[async_trait]
impl PluginHandler for StaticReplyHandler {
    async fn handle(&self, event: &mut MessageEvent) -> Result<PluginControl> {
        event.set_result(MessageEventResult::general(self.reply));
        Ok(PluginControl::Continue)
    }
}

#[tokio::test]
async fn command_plugin_runs_before_provider_fallback() {
    let (event_tx, event_rx) = mpsc::channel(8);
    let sink = Arc::new(RecordingSink::default());
    let platform = MockPlatform::new(event_tx, sink.clone());
    let provider = Arc::new(MockChatProvider::new("provider-response"));

    let mut plugins = PluginRegistry::new();
    plugins.register_handler(
        RegisteredHandler::new(
            HandlerMetadata::new("builtin", "ping", PluginEventType::AdapterMessage),
            Arc::new(StaticReplyHandler { reply: "pong" }),
        )
        .with_filter(CommandFilter::new("ping")),
    );

    let scheduler = Arc::new(
        PipelineScheduler::new(
            PipelineContext::with_chat_provider(provider).with_plugin_registry(Arc::new(plugins)),
        )
        .with_stage(PluginStage)
        .with_stage(ProviderStage)
        .with_stage(RespondStage),
    );
    let mut event_bus = EventBus::new(event_rx, scheduler);

    platform
        .emit_text("event-1", "conversation-1", "user-1", "/ping")
        .await
        .expect("mock platform should submit event");
    event_bus
        .run_once()
        .await
        .expect("event bus should dispatch");

    let sent = sink.messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "pong");
}
