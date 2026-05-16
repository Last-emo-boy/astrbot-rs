use std::sync::Arc;

use astrbot_core::EventExecutor;
use astrbot_pipeline::{
    PipelineContext, PipelineScheduler, ProviderFallbackConfig,
    stages::{ProcessStage, RespondStage},
};
use astrbot_platform::RecordingSink;

use crate::support::{CapturingProvider, direct_event};

#[tokio::test]
async fn process_stage_disabled_provider_fallback_skips_provider() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_provider_fallback(ProviderFallbackConfig::disabled()),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("hello", sink.clone()))
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn process_stage_require_wake_blocks_implicit_provider_fallback() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_provider_fallback(ProviderFallbackConfig::default().require_wake(true)),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("hello", sink.clone()))
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn process_stage_skips_provider_when_provider_is_absent() {
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(PipelineContext::new())
        .with_stage(ProcessStage)
        .with_stage(RespondStage);

    scheduler
        .execute(direct_event("hello", sink.clone()))
        .await
        .expect("scheduler should execute");

    assert!(sink.messages().await.is_empty());
}
