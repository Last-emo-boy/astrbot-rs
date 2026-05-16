use std::sync::Arc;

use astrbot_core::{
    EventExecutor, MessageChain, MessageEvent, MessageEventResult, MessageSender, MessageSession,
    Result,
};
use astrbot_pipeline::{
    PipelineContext, PipelineControl, PipelineScheduler, PipelineStage, ResultDecorateConfig,
    stages::{RespondStage, ResultDecorateStage},
};
use astrbot_platform::RecordingSink;
use async_trait::async_trait;

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
