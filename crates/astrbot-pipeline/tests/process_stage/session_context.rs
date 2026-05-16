use std::sync::Arc;

use astrbot_core::{EventExecutor, ProviderContentPart, ProviderContextMessage};
use astrbot_pipeline::{
    PipelineContext, PipelineScheduler,
    stages::{ProcessStage, RespondStage},
};
use astrbot_platform::RecordingSink;

use crate::support::{CapturingProvider, StaticSessionContextPort, direct_event};

#[tokio::test]
async fn process_stage_injects_session_context_into_provider_request() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone()).with_session_context_port(Arc::new(
            StaticSessionContextPort::new(vec![ProviderContextMessage::text(
                "assistant",
                "previous answer",
            )]),
        )),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("continue", sink.clone()))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].prompt, "continue");
    assert_eq!(requests[0].contexts.len(), 1);
    assert_eq!(requests[0].contexts[0].role, "assistant");
    assert_eq!(
        requests[0].contexts[0].parts,
        vec![ProviderContentPart::text("previous answer")]
    );
}
