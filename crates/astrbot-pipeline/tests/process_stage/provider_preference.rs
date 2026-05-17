use std::sync::Arc;

use astrbot_core::EventExecutor;
use astrbot_pipeline::{
    InMemoryProviderPreferencePort, PipelineContext, PipelineScheduler,
    ScopedProviderPreferencePort,
    stages::{ProcessStage, RespondStage},
};
use astrbot_platform::RecordingSink;
use astrbot_session::ProviderCapability;
use astrbot_storage::InMemorySessionRuleRepository;

use crate::support::{CapturingProvider, direct_event};

#[tokio::test]
async fn process_stage_applies_session_provider_preference_to_provider_request() {
    let provider = Arc::new(CapturingProvider::default());
    let preference = Arc::new(InMemoryProviderPreferencePort::new());
    preference
        .set_preferred_chat_provider("conversation-1", "session-provider")
        .await
        .expect("provider preference should be stored");
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_provider_preference_port(preference),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("hello", sink.clone()))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].provider_id.as_deref(), Some("session-provider"));
}

#[tokio::test]
async fn process_stage_reads_chat_provider_preference_from_scoped_session_rules() {
    let provider = Arc::new(CapturingProvider::default());
    let repository = Arc::new(InMemorySessionRuleRepository::new());
    let preference = Arc::new(ScopedProviderPreferencePort::new(repository));
    preference
        .set_preferred_provider(
            "mock:conversation-1",
            ProviderCapability::ChatCompletion,
            "scoped-provider",
        )
        .await
        .expect("provider preference should be stored");
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_provider_preference_port(preference),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("hello", sink.clone()))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].provider_id.as_deref(), Some("scoped-provider"));
}
