use std::sync::Arc;

use astrbot_core::{
    AstrbotError, EventExecutor, MessageChain, MessageComponent, MessageEvent, MessageEventResult,
    MessageSender, MessageSession, MessageStream, Result,
};
use astrbot_pipeline::{
    ContentSafetyConfig, KeywordContentSafetyStrategy, PipelineContext, PipelineControl,
    PipelineScheduler, PipelineStage, ResultDecorateConfig, ResultFileService,
    TextToImageDecorateConfig, TextToSpeechDecorateConfig,
    stages::{RespondStage, ResultDecorateStage},
};
use astrbot_platform::RecordingSink;
use astrbot_provider::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};
use astrbot_render::{
    RenderArtifact, RenderFormat, RenderMode, RenderStrategy, T2iRenderRequest, T2iRenderResult,
    T2iRenderer, TemplateCatalog, TemplateName, TemplateRenderer,
};
use async_trait::async_trait;
use tokio::sync::Mutex;

#[tokio::test]
async fn result_decorate_stage_prefixes_llm_reply_before_respond() {
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::new()
            .with_result_decorate(ResultDecorateConfig::default().with_reply_prefix("[bot] ")),
    )
    .with_stage(SetResultStage::llm("hello"))
    .with_stage(ResultDecorateStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("input", sink.clone()))
        .await
        .expect("scheduler should execute");

    let sent = sink.messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "[bot] hello");
}

#[tokio::test]
async fn result_decorate_stage_can_limit_prefix_to_llm_results() {
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::new().with_result_decorate(
            ResultDecorateConfig::default()
                .with_reply_prefix("[bot] ")
                .only_llm_result(true),
        ),
    )
    .with_stage(SetResultStage::general("pong"))
    .with_stage(ResultDecorateStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("input", sink.clone()))
        .await
        .expect("scheduler should execute");

    let sent = sink.messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "pong");
}

#[tokio::test]
async fn result_decorate_stage_ignores_empty_results() {
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::new()
            .with_result_decorate(ResultDecorateConfig::default().with_reply_prefix("[bot] ")),
    )
    .with_stage(SetResultStage::llm(MessageChain::default()))
    .with_stage(ResultDecorateStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("input", sink.clone()))
        .await
        .expect("scheduler should execute");

    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn result_decorate_stage_converts_llm_plain_to_record_with_tts_provider() {
    let sink = Arc::new(RecordingSink::default());
    let tts = Arc::new(RecordingTtsProvider::new("voice.wav"));
    let scheduler = PipelineScheduler::new(
        PipelineContext::new()
            .with_text_to_speech_provider(tts.clone())
            .with_result_decorate(
                ResultDecorateConfig::default()
                    .with_reply_prefix("[bot] ")
                    .with_tts(TextToSpeechDecorateConfig::enabled()),
            ),
    )
    .with_stage(SetResultStage::llm("hello"))
    .with_stage(ResultDecorateStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("input", sink.clone()))
        .await
        .expect("scheduler should execute");

    let sent = sink.messages().await;
    assert_eq!(
        sent[0].chain.components(),
        &[MessageComponent::record("voice.wav")]
    );
    assert_eq!(tts.requests().await[0].text, "[bot] hello");
}

#[tokio::test]
async fn result_decorate_stage_falls_back_to_text_when_tts_fails() {
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::new()
            .with_text_to_speech_provider(Arc::new(FailingTtsProvider))
            .with_result_decorate(
                ResultDecorateConfig::default()
                    .with_tts(TextToSpeechDecorateConfig::enabled().with_dual_output(true)),
            ),
    )
    .with_stage(SetResultStage::llm("hello"))
    .with_stage(ResultDecorateStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("input", sink.clone()))
        .await
        .expect("scheduler should execute");

    let sent = sink.messages().await;
    assert_eq!(sent[0].chain.plain_text(), "hello");
}

#[tokio::test]
async fn result_decorate_stage_renders_long_plain_with_active_t2i_template() {
    let root = std::env::temp_dir().join(format!("astrbot_pipeline_t2i_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let template_dir = root.join("templates");
    let output_dir = root.join("output");
    let catalog = TemplateCatalog::new(&template_dir);
    let template = TemplateName::new("card").expect("template name");
    catalog
        .put_user_template(&template, "Rendered {{ text }}")
        .expect("template should write");
    let renderer = Arc::new(TemplateRenderer::new(catalog, &output_dir));
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::new()
            .with_t2i_renderer(renderer)
            .with_result_decorate(
                ResultDecorateConfig::default().with_t2i(
                    TextToImageDecorateConfig::enabled()
                        .with_word_threshold(50)
                        .with_strategy(RenderStrategy::LocalOnly)
                        .with_mode(RenderMode::File)
                        .with_active_template(template),
                ),
            ),
    )
    .with_stage(SetResultStage::llm("x".repeat(80)))
    .with_stage(ResultDecorateStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("input", sink.clone()))
        .await
        .expect("scheduler should execute");

    let sent = sink.messages().await;
    let [MessageComponent::Image { url }] = sent[0].chain.components() else {
        panic!("single image component expected");
    };
    let rendered = std::fs::read_to_string(url).expect("rendered artifact should exist");
    assert!(rendered.contains("Rendered"));
    assert!(rendered.contains(&"x".repeat(80)));

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn result_decorate_stage_uses_file_service_for_t2i_artifacts() {
    let sink = Arc::new(RecordingSink::default());
    let file_service = Arc::new(StaticFileService::new("https://files.test/t2i.png"));
    let scheduler = PipelineScheduler::new(
        PipelineContext::new()
            .with_t2i_renderer(Arc::new(StaticT2iRenderer::file("local.png")))
            .with_result_file_service(file_service.clone())
            .with_result_decorate(
                ResultDecorateConfig::default().with_t2i(
                    TextToImageDecorateConfig::enabled()
                        .with_word_threshold(50)
                        .with_mode(RenderMode::File)
                        .with_file_service(true),
                ),
            ),
    )
    .with_stage(SetResultStage::llm("x".repeat(80)))
    .with_stage(ResultDecorateStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("input", sink.clone()))
        .await
        .expect("scheduler should execute");

    let sent = sink.messages().await;
    assert_eq!(
        sent[0].chain.components(),
        &[MessageComponent::image("https://files.test/t2i.png")]
    );
    assert_eq!(file_service.artifacts().await[0].value, "local.png");
}

#[tokio::test]
async fn result_decorate_stage_blocks_unsafe_llm_reply_before_tts_or_t2i() {
    let sink = Arc::new(RecordingSink::default());
    let tts = Arc::new(RecordingTtsProvider::new("voice.wav"));
    let scheduler = PipelineScheduler::new(
        PipelineContext::new()
            .with_text_to_speech_provider(tts.clone())
            .with_content_safety(
                ContentSafetyConfig::default()
                    .with_strategy(Arc::new(KeywordContentSafetyStrategy::new(["bad"])))
                    .with_rejection_message("blocked"),
            )
            .with_result_decorate(
                ResultDecorateConfig::default().with_tts(TextToSpeechDecorateConfig::enabled()),
            ),
    )
    .with_stage(SetResultStage::llm("bad reply"))
    .with_stage(ResultDecorateStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("input", sink.clone()))
        .await
        .expect("scheduler should execute");

    let sent = sink.messages().await;
    assert_eq!(sent[0].chain.plain_text(), "blocked");
    assert!(tts.requests().await.is_empty());
}

#[tokio::test]
async fn result_decorate_stage_keeps_streaming_and_stopped_results_respondable() {
    let sink = Arc::new(RecordingSink::default());
    let streaming_scheduler = PipelineScheduler::new(PipelineContext::new())
        .with_stage(SetResultStage::streaming("chunk"))
        .with_stage(ResultDecorateStage)
        .with_stage(RespondStage);

    streaming_scheduler
        .execute(direct_event("input", sink.clone()))
        .await
        .expect("streaming scheduler should execute");

    assert_eq!(sink.streaming_messages().await.len(), 1);

    let stopped_sink = Arc::new(RecordingSink::default());
    let stopped_scheduler = PipelineScheduler::new(PipelineContext::new())
        .with_stage(SetResultStage::llm(MessageChain::plain("stop")).stop())
        .with_stage(ResultDecorateStage)
        .with_stage(RespondStage)
        .with_stage(PanicStage);

    stopped_scheduler
        .execute(direct_event("input", stopped_sink.clone()))
        .await
        .expect("stopped result should still reach respond");

    let sent = stopped_sink.messages().await;
    assert_eq!(sent[0].chain.plain_text(), "stop");
}

struct SetResultStage {
    result: MessageEventResult,
}

impl SetResultStage {
    fn llm(chain: impl Into<MessageChain>) -> Self {
        Self {
            result: MessageEventResult::llm(chain),
        }
    }

    fn general(chain: impl Into<MessageChain>) -> Self {
        Self {
            result: MessageEventResult::general(chain),
        }
    }

    fn streaming(chain: impl Into<MessageChain>) -> Self {
        Self {
            result: MessageEventResult::streaming(MessageStream::from_chunk(chain.into())),
        }
    }

    fn stop(mut self) -> Self {
        self.result = self.result.stop();
        self
    }
}

#[async_trait]
impl PipelineStage for SetResultStage {
    fn name(&self) -> &str {
        "set_result"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        _ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        event.set_result(self.result.clone());
        Ok(PipelineControl::Continue)
    }
}

#[derive(Default)]
struct PanicStage;

#[async_trait]
impl PipelineStage for PanicStage {
    fn name(&self) -> &str {
        "panic"
    }

    async fn handle(
        &self,
        _event: &mut MessageEvent,
        _ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        panic!("stopped result should stop before this stage")
    }
}

struct RecordingTtsProvider {
    audio_path: String,
    requests: Mutex<Vec<TextToSpeechRequest>>,
}

impl RecordingTtsProvider {
    fn new(audio_path: impl Into<String>) -> Self {
        Self {
            audio_path: audio_path.into(),
            requests: Mutex::new(Vec::new()),
        }
    }

    async fn requests(&self) -> Vec<TextToSpeechRequest> {
        self.requests.lock().await.clone()
    }
}

#[async_trait]
impl TextToSpeechProvider for RecordingTtsProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        self.requests.lock().await.push(request);
        Ok(TextToSpeechResponse::new(self.audio_path.clone()))
    }
}

struct FailingTtsProvider;

#[async_trait]
impl TextToSpeechProvider for FailingTtsProvider {
    async fn synthesize(&self, _request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        Err(AstrbotError::Provider("tts failed".to_string()))
    }
}

struct StaticT2iRenderer {
    artifact: RenderArtifact,
}

impl StaticT2iRenderer {
    fn file(path: impl Into<String>) -> Self {
        Self {
            artifact: RenderArtifact::file(path.into(), RenderFormat::Png),
        }
    }
}

#[async_trait]
impl T2iRenderer for StaticT2iRenderer {
    async fn render(&self, request: T2iRenderRequest) -> Result<T2iRenderResult> {
        Ok(T2iRenderResult {
            artifact: self.artifact.clone(),
            template_name: request.options.template_name,
            strategy_used: request.options.strategy,
        })
    }
}

struct StaticFileService {
    public_url: String,
    artifacts: Mutex<Vec<RenderArtifact>>,
}

impl StaticFileService {
    fn new(public_url: impl Into<String>) -> Self {
        Self {
            public_url: public_url.into(),
            artifacts: Mutex::new(Vec::new()),
        }
    }

    async fn artifacts(&self) -> Vec<RenderArtifact> {
        self.artifacts.lock().await.clone()
    }
}

#[async_trait]
impl ResultFileService for StaticFileService {
    async fn public_url(&self, artifact: &RenderArtifact) -> Result<Option<String>> {
        self.artifacts.lock().await.push(artifact.clone());
        Ok(Some(self.public_url.clone()))
    }
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
