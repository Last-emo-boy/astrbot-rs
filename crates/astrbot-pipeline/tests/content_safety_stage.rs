use std::sync::Arc;

use astrbot_core::{
    EventExecutor, MessageChain, MessageEvent, MessageEventResult, MessageSender, MessageSession,
    Result,
};
use astrbot_pipeline::{
    ContentSafetyConfig, KeywordContentSafetyStrategy, PipelineContext, PipelineScheduler,
    stages::{ContentSafetyCheckStage, PluginStage, ProviderStage, RespondStage},
};
use astrbot_platform::RecordingSink;
use astrbot_plugin::{
    CommandFilter, HandlerMetadata, PluginControl, PluginEventType, PluginHandler, PluginRegistry,
    RegisteredHandler,
};
use astrbot_provider::{ChatProvider, ChatRequest, ChatResponse};
use async_trait::async_trait;
use tokio::sync::Mutex;

#[tokio::test]
async fn blocked_wake_message_sends_rejection_and_skips_provider() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone()).with_content_safety(
            keyword_safety(["bad"]).with_rejection_message("blocked by content safety"),
        ),
    )
    .with_stage(ContentSafetyCheckStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);
    let mut event = direct_event("bad request", sink.clone());
    event.mark_wake(true);

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert_eq!(
        sink.messages().await[0].chain.plain_text(),
        "blocked by content safety"
    );
}

#[tokio::test]
async fn blocked_non_wake_message_stops_silently() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_content_safety(keyword_safety(["bad"])),
    )
    .with_stage(ContentSafetyCheckStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("bad request", sink.clone()))
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn safe_message_reaches_provider() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_content_safety(keyword_safety(["bad"])),
    )
    .with_stage(ContentSafetyCheckStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("ordinary request", sink.clone()))
        .await
        .expect("scheduler should execute");

    assert_eq!(provider.requests.lock().await.len(), 1);
    assert_eq!(sink.messages().await[0].chain.plain_text(), "mock-response");
}

#[tokio::test]
async fn content_safety_result_prevents_plugin_override() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let mut plugins = PluginRegistry::new();
    plugins.register_handler(
        RegisteredHandler::new(
            HandlerMetadata::new("builtin", "bad", PluginEventType::AdapterMessage),
            Arc::new(StaticReplyHandler {
                reply: "plugin-response",
            }),
        )
        .with_filter(CommandFilter::new("bad")),
    );
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_content_safety(
                keyword_safety(["bad"]).with_rejection_message("blocked by content safety"),
            )
            .with_plugin_registry(Arc::new(plugins)),
    )
    .with_stage(ContentSafetyCheckStage)
    .with_stage(PluginStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);
    let mut event = direct_event("/bad", sink.clone());
    event.mark_wake(true);

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert_eq!(
        sink.messages().await[0].chain.plain_text(),
        "blocked by content safety"
    );
}

fn keyword_safety<I, S>(keywords: I) -> ContentSafetyConfig
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    ContentSafetyConfig::default()
        .with_strategy(Arc::new(KeywordContentSafetyStrategy::new(keywords)))
}

fn direct_event(text: impl Into<String>, sink: Arc<RecordingSink>) -> MessageEvent {
    MessageEvent::new(
        "event-1",
        "mock",
        "Mock Platform",
        MessageSession::new("mock", "conversation-1"),
        MessageSender::new("user-1", None),
        MessageChain::plain(text),
        sink,
    )
}

#[derive(Default)]
struct CapturingProvider {
    requests: Mutex<Vec<ChatRequest>>,
}

#[async_trait]
impl ChatProvider for CapturingProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        self.requests.lock().await.push(request);
        Ok(ChatResponse::text("mock-response"))
    }
}

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
