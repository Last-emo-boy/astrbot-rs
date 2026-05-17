use std::sync::Arc;

use astrbot_core::{
    EventExecutor, MessageChain, MessageComponent, MessageEvent, MessageSender, MessageSession,
    Result,
};
use astrbot_pipeline::{
    PipelineContext, PipelineScheduler, PreAckConfig, PreAckReactionSink, PrefixPathMapper,
    PrefixPathMapping, PreprocessConfig, SpeechToTextPreprocessConfig, stages::PreprocessStage,
};
use astrbot_platform::RecordingSink;
use astrbot_provider::{
    MockSpeechToTextProvider, SpeechToTextProvider, SpeechToTextRequest, SpeechToTextResponse,
};
use async_trait::async_trait;
use tokio::sync::Mutex;

#[tokio::test]
async fn preprocess_stage_sends_optional_pre_ack_for_wake_commands() {
    let sink = Arc::new(RecordingSink::default());
    let reaction_sink = Arc::new(RecordingReactionSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::new().with_preprocess(
            PreprocessConfig::default()
                .with_pre_ack(
                    PreAckConfig::enabled(["eyes"])
                        .with_supported_platforms(["telegram", "test-platform"]),
                )
                .with_pre_ack_sink(reaction_sink.clone()),
        ),
    )
    .with_stage(PreprocessStage);
    let mut event = direct_event("telegram", MessageChain::plain("hello"), sink);
    event.mark_wake(true);

    scheduler
        .execute(event)
        .await
        .expect("preprocess should run");

    assert_eq!(
        reaction_sink.reactions.lock().await.as_slice(),
        &[("event-1".to_string(), "eyes".to_string())]
    );
}

#[tokio::test]
async fn preprocess_stage_maps_record_and_image_paths_before_later_stages() {
    let sink = Arc::new(RecordingSink::default());
    let mapper = Arc::new(PrefixPathMapper::new([PrefixPathMapping::new(
        "/host/media",
        "/container/media",
    )]));
    let captured_components = Arc::new(Mutex::new(Vec::new()));
    let capture = MessageCaptureStage::new(captured_components.clone());
    let scheduler = PipelineScheduler::new(
        PipelineContext::new()
            .with_preprocess(PreprocessConfig::default().with_path_mapper(mapper)),
    )
    .with_stage(PreprocessStage)
    .with_stage(capture);

    scheduler
        .execute(direct_event(
            "mock",
            MessageChain::new(vec![
                MessageComponent::image("file:///host/media/image.png"),
                MessageComponent::record("/host/media/audio.ogg"),
            ]),
            sink,
        ))
        .await
        .expect("preprocess should map paths");

    let captured = captured_components.lock().await.clone();
    assert_eq!(
        captured,
        vec![
            MessageComponent::image("/container/media/image.png"),
            MessageComponent::record("/container/media/audio.ogg"),
        ]
    );
}

#[tokio::test]
async fn preprocess_stage_converts_record_to_plain_text_through_stt_port() {
    let sink = Arc::new(RecordingSink::default());
    let provider = Arc::new(MockSpeechToTextProvider::new("transcribed voice"));
    let captured_components = Arc::new(Mutex::new(Vec::new()));
    let capture = MessageCaptureStage::new(captured_components.clone());
    let scheduler = PipelineScheduler::new(
        PipelineContext::new().with_preprocess(
            PreprocessConfig::default()
                .with_speech_to_text(SpeechToTextPreprocessConfig::enabled())
                .with_speech_to_text_provider(provider),
        ),
    )
    .with_stage(PreprocessStage)
    .with_stage(capture);

    scheduler
        .execute(direct_event(
            "mock",
            MessageChain::new(vec![MessageComponent::record("file:///tmp/audio.ogg")]),
            sink,
        ))
        .await
        .expect("preprocess should transcribe record");

    let captured = captured_components.lock().await.clone();
    assert_eq!(captured, vec![MessageComponent::plain("transcribed voice")]);
}

#[tokio::test]
async fn preprocess_stage_passes_configured_stt_provider_id_without_concrete_manager() {
    let sink = Arc::new(RecordingSink::default());
    let provider = Arc::new(CapturingSpeechToTextProvider::new("speech text"));
    let scheduler = PipelineScheduler::new(
        PipelineContext::new().with_preprocess(
            PreprocessConfig::default()
                .with_speech_to_text(
                    SpeechToTextPreprocessConfig::enabled().with_provider_id("stt-session"),
                )
                .with_speech_to_text_provider(provider.clone()),
        ),
    )
    .with_stage(PreprocessStage);

    scheduler
        .execute(direct_event(
            "mock",
            MessageChain::new(vec![MessageComponent::record("file:///tmp/audio.ogg")]),
            sink,
        ))
        .await
        .expect("preprocess should call STT provider");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].provider_id.as_deref(), Some("stt-session"));
    assert_eq!(requests[0].audio_url, "/tmp/audio.ogg");
}

fn direct_event(
    platform_id: impl Into<String>,
    message: MessageChain,
    sink: Arc<RecordingSink>,
) -> MessageEvent {
    let platform_id = platform_id.into();
    MessageEvent::new(
        "event-1",
        platform_id.clone(),
        platform_id.clone(),
        MessageSession::new(platform_id, "conversation-1"),
        MessageSender::new("user-1", None),
        message,
        sink,
    )
}

#[derive(Default)]
struct RecordingReactionSink {
    reactions: Mutex<Vec<(String, String)>>,
}

#[async_trait]
impl PreAckReactionSink for RecordingReactionSink {
    async fn react(&self, event: &MessageEvent, reaction: &str) -> Result<()> {
        self.reactions
            .lock()
            .await
            .push((event.id.clone(), reaction.to_string()));
        Ok(())
    }
}

struct MessageCaptureStage {
    components: Arc<Mutex<Vec<MessageComponent>>>,
}

impl MessageCaptureStage {
    fn new(components: Arc<Mutex<Vec<MessageComponent>>>) -> Self {
        Self { components }
    }
}

#[async_trait]
impl astrbot_pipeline::PipelineStage for MessageCaptureStage {
    fn name(&self) -> &str {
        "capture"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        _ctx: &PipelineContext,
    ) -> Result<astrbot_pipeline::PipelineControl> {
        *self.components.lock().await = event.message.components().to_vec();
        Ok(astrbot_pipeline::PipelineControl::Continue)
    }
}

struct CapturingSpeechToTextProvider {
    text: String,
    requests: Mutex<Vec<SpeechToTextRequest>>,
}

impl CapturingSpeechToTextProvider {
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            requests: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl SpeechToTextProvider for CapturingSpeechToTextProvider {
    async fn transcribe(&self, request: SpeechToTextRequest) -> Result<SpeechToTextResponse> {
        self.requests.lock().await.push(request);
        Ok(SpeechToTextResponse::new(self.text.clone()))
    }
}
