use std::sync::Arc;

use astrbot_core::{
    EventExecutor, MessageChain, MessageComponent, MessageEvent, MessageSender, MessageSession,
};
use astrbot_pipeline::{
    PipelineContext, PipelineScheduler, WakeCheckConfig,
    stages::{ProviderStage, RespondStage, WakeCheckStage},
};
use astrbot_platform::RecordingSink;
use astrbot_provider::{ChatProvider, ChatRequest, ChatResponse};
use async_trait::async_trait;
use tokio::sync::Mutex;

#[tokio::test]
async fn group_message_with_wake_prefix_reaches_provider_without_prefix() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = scheduler_with_wake_config(
        provider.clone(),
        WakeCheckConfig::default().with_wake_prefixes(["bot"]),
    );
    let event = group_event(MessageChain::plain("bot hello"), sink.clone());

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].prompt, "hello");
    assert_eq!(sink.messages().await[0].chain.plain_text(), "mock-response");
}

#[tokio::test]
async fn group_message_without_wake_marker_stops_before_provider() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = scheduler_with_wake_config(
        provider.clone(),
        WakeCheckConfig::default().with_wake_prefixes(["bot"]),
    );
    let event = group_event(MessageChain::plain("hello"), sink.clone());

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn group_message_with_bot_mention_reaches_provider() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = scheduler_with_wake_config(
        provider.clone(),
        WakeCheckConfig::default().with_bot_self_id("bot-1"),
    );
    let event = group_event(
        MessageChain::new(vec![
            MessageComponent::mention("bot-1"),
            MessageComponent::plain("hello"),
        ]),
        sink.clone(),
    );

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].prompt, "hello");
}

#[tokio::test]
async fn group_prefix_after_non_bot_leading_mention_does_not_wake() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = scheduler_with_wake_config(
        provider.clone(),
        WakeCheckConfig::default()
            .with_wake_prefixes(["bot"])
            .with_bot_self_id("bot-1"),
    );
    let event = group_event(
        MessageChain::new(vec![
            MessageComponent::mention("other-user"),
            MessageComponent::plain("bot hello"),
        ]),
        sink.clone(),
    );

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn direct_message_wakes_by_default() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = scheduler_with_wake_config(provider.clone(), WakeCheckConfig::default());
    let event = MessageEvent::new(
        "event-1",
        "mock",
        "Mock Platform",
        MessageSession::new("mock", "conversation-1"),
        MessageSender::new("user-1", None),
        MessageChain::plain("hello"),
        sink.clone(),
    );

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].prompt, "hello");
}

fn scheduler_with_wake_config(
    provider: Arc<CapturingProvider>,
    wake_check: WakeCheckConfig,
) -> PipelineScheduler {
    PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider).with_wake_check(wake_check),
    )
    .with_stage(WakeCheckStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage)
}

fn group_event(message: MessageChain, sink: Arc<RecordingSink>) -> MessageEvent {
    MessageEvent::new(
        "event-1",
        "mock",
        "Mock Platform",
        MessageSession::group("mock", "group-1"),
        MessageSender::new("user-1", None),
        message,
        sink,
    )
    .with_self_id("bot-1")
}

#[derive(Default)]
struct CapturingProvider {
    requests: Mutex<Vec<ChatRequest>>,
}

#[async_trait]
impl ChatProvider for CapturingProvider {
    async fn chat(&self, request: ChatRequest) -> astrbot_core::Result<ChatResponse> {
        self.requests.lock().await.push(request);
        Ok(ChatResponse::text("mock-response"))
    }
}
