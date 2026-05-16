use std::sync::Arc;

use astrbot_core::{
    EventBus, MessageChain, MessageComponent, MessageEvent, MessageSender, MessageSession,
};
use astrbot_pipeline::{
    PipelineContext, PipelineScheduler,
    stages::{ProviderStage, RespondStage},
};
use astrbot_platform::{MockPlatform, RecordingSink};
use astrbot_provider::{ChatProvider, ChatRequest, ChatResponse, MockChatProvider};
use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

#[tokio::test]
async fn mock_platform_message_reaches_provider_and_responds() {
    let (event_tx, event_rx) = mpsc::channel(8);
    let sink = Arc::new(RecordingSink::default());
    let platform = MockPlatform::new(event_tx, sink.clone());
    let provider = Arc::new(MockChatProvider::new("mock-response"));

    let scheduler = Arc::new(
        PipelineScheduler::new(PipelineContext::with_chat_provider(provider))
            .with_stage(ProviderStage)
            .with_stage(RespondStage),
    );
    let mut event_bus = EventBus::new(event_rx, scheduler);

    platform
        .emit_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("mock platform should submit event");

    assert!(
        event_bus
            .run_once()
            .await
            .expect("event bus should dispatch"),
        "one event should be processed"
    );

    let sent = sink.messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].session.conversation_id, "conversation-1");
    assert_eq!(sent[0].chain.plain_text(), "mock-response");
}

#[tokio::test]
async fn image_only_message_reaches_provider_with_image_urls() {
    let (event_tx, event_rx) = mpsc::channel(8);
    let sink = Arc::new(RecordingSink::default());
    let provider = Arc::new(CapturingProvider::default());

    let scheduler = Arc::new(
        PipelineScheduler::new(PipelineContext::with_chat_provider(provider.clone()))
            .with_stage(ProviderStage)
            .with_stage(RespondStage),
    );
    let mut event_bus = EventBus::new(event_rx, scheduler);

    let event = MessageEvent::new(
        "event-1",
        "mock",
        "Mock Platform",
        MessageSession::new("mock", "conversation-1"),
        MessageSender::new("user-1", None),
        MessageChain::new(vec![MessageComponent::image(
            "https://example.test/image.png",
        )]),
        sink.clone(),
    );
    event_tx
        .send(event)
        .await
        .expect("image event should enter queue");
    event_bus
        .run_once()
        .await
        .expect("event bus should dispatch");

    let request = provider
        .request
        .lock()
        .await
        .clone()
        .expect("provider should receive request");
    assert_eq!(request.prompt, "");
    assert_eq!(
        request.image_urls,
        vec!["https://example.test/image.png".to_string()]
    );

    let sent = sink.messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "mock-response");
}

#[tokio::test]
async fn non_image_media_only_message_does_not_call_provider() {
    let (event_tx, event_rx) = mpsc::channel(8);
    let sink = Arc::new(RecordingSink::default());
    let provider = Arc::new(CapturingProvider::default());

    let scheduler = Arc::new(
        PipelineScheduler::new(PipelineContext::with_chat_provider(provider.clone()))
            .with_stage(ProviderStage)
            .with_stage(RespondStage),
    );
    let mut event_bus = EventBus::new(event_rx, scheduler);

    let event = MessageEvent::new(
        "event-1",
        "mock",
        "Mock Platform",
        MessageSession::new("mock", "conversation-1"),
        MessageSender::new("user-1", None),
        MessageChain::new(vec![
            MessageComponent::reply("message-1", "quoted text"),
            MessageComponent::record("https://example.test/audio.ogg"),
            MessageComponent::video("https://example.test/video.mp4"),
            MessageComponent::file("report.pdf", "https://example.test/report.pdf"),
        ]),
        sink.clone(),
    );
    event_tx
        .send(event)
        .await
        .expect("media event should enter queue");
    event_bus
        .run_once()
        .await
        .expect("event bus should dispatch");

    assert!(provider.request.lock().await.is_none());
    assert!(sink.messages().await.is_empty());
}

#[derive(Default)]
struct CapturingProvider {
    request: Mutex<Option<ChatRequest>>,
}

#[async_trait]
impl ChatProvider for CapturingProvider {
    async fn chat(&self, request: ChatRequest) -> astrbot_core::Result<ChatResponse> {
        *self.request.lock().await = Some(request);
        Ok(ChatResponse::text("mock-response"))
    }
}
