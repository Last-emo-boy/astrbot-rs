use std::sync::Arc;

use astrbot_core::EventExecutor;
use astrbot_pipeline::{
    PipelineContext, PipelineScheduler, ProviderFallbackConfig,
    stages::{ProcessStage, RespondStage},
};
use astrbot_platform::RecordingSink;

use crate::support::{FailingProvider, direct_event};

#[tokio::test]
async fn process_stage_provider_error_can_send_generic_response() {
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(Arc::new(FailingProvider)).with_provider_fallback(
            ProviderFallbackConfig::default().with_error_message("provider unavailable"),
        ),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("hello", sink.clone()))
        .await
        .expect("provider error should map to configured response");

    let sent = sink.messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "provider unavailable");
}
