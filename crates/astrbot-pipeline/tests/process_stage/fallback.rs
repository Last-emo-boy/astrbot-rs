use std::sync::Arc;

use astrbot_agent::{AgentHookEvent, AgentHookEventKind};
use astrbot_core::EventExecutor;
use astrbot_pipeline::{
    PipelineContext, PipelineScheduler, ProviderFallbackConfig, WakeCheckConfig,
    stages::{ProcessStage, RespondStage},
};
use astrbot_platform::RecordingSink;

use crate::support::{
    CapturingAgentHook, CapturingProvider, CapturingProviderRequestHook, direct_event,
};

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

#[tokio::test]
async fn process_stage_routes_agent_lifecycle_through_hook_port() {
    let provider = Arc::new(CapturingProvider::default());
    let hook = Arc::new(CapturingAgentHook::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider).with_agent_run_hook(hook.clone()),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("hello", sink.clone()))
        .await
        .expect("scheduler should execute");

    let events = hook.events.lock().await;
    assert_eq!(
        events.iter().map(AgentHookEvent::kind).collect::<Vec<_>>(),
        vec![
            AgentHookEventKind::AgentBegin,
            AgentHookEventKind::WaitingLlmRequest,
            AgentHookEventKind::LlmRequest,
            AgentHookEventKind::AgentDone
        ]
    );
    let AgentHookEvent::AgentDone(done) = &events[3] else {
        panic!("fourth hook should be agent done");
    };
    assert_eq!(done.lifecycle.session_id, "conversation-1");
    assert_eq!(done.chain.plain_text(), "mock-response");
}

#[tokio::test]
async fn process_stage_trims_provider_wake_prefix_after_bot_wake_prefix_normalization() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_wake_check(WakeCheckConfig::default().with_wake_prefixes(["/"]))
            .with_provider_fallback(
                ProviderFallbackConfig::default().with_provider_wake_prefixes("/llm", ["/"]),
            ),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("llm explain", sink.clone()))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].prompt, "explain");
    assert_eq!(requests[0].wake_prefix.as_deref(), Some("llm"));
}

#[tokio::test]
async fn process_stage_skips_implicit_request_without_provider_wake_prefix() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone()).with_provider_fallback(
            ProviderFallbackConfig::default().with_provider_wake_prefix("llm"),
        ),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("plain prompt", sink.clone()))
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn process_stage_provider_request_hook_can_modify_llm_request() {
    let provider = Arc::new(CapturingProvider::default());
    let hook = Arc::new(CapturingProviderRequestHook {
        rewrite_prompt: Some("hook rewrite".to_string()),
        ..CapturingProviderRequestHook::default()
    });
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_provider_request_hook(hook.clone()),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("hello", sink.clone()))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].prompt, "hook rewrite");
    assert_eq!(hook.seen.lock().await[0].prompt.as_deref(), Some("hello"));
}

#[tokio::test]
async fn process_stage_provider_request_hook_can_stop_before_provider_call() {
    let provider = Arc::new(CapturingProvider::default());
    let hook = Arc::new(CapturingProviderRequestHook {
        stop: true,
        ..CapturingProviderRequestHook::default()
    });
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_provider_request_hook(hook.clone()),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("hello", sink.clone()))
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert_eq!(hook.seen.lock().await.len(), 1);
    assert!(sink.messages().await.is_empty());
}
