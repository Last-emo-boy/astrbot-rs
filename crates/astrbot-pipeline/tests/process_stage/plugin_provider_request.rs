use astrbot_agent::{AgentHookEvent, AgentHookEventKind};
use std::sync::Arc;

use astrbot_core::{EventExecutor, MessageChain, MessageComponent};
use astrbot_pipeline::{
    PipelineContext, PipelineScheduler, ProviderFallbackConfig,
    stages::{ProcessStage, RespondStage},
};
use astrbot_platform::RecordingSink;
use astrbot_plugin::{
    CommandFilter, HandlerMetadata, PluginEventType, PluginRegistry, RegisteredHandler,
};

use crate::support::{
    CapturingAgentHook, CapturingProvider, NoopHandler, ProviderRequestHandler, StaticReplyHandler,
    direct_event, event_with_chain,
};

#[tokio::test]
async fn process_stage_plugin_result_suppresses_provider_fallback() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let mut plugins = PluginRegistry::new();
    plugins.register_handler(
        RegisteredHandler::new(
            HandlerMetadata::new("builtin", "ping", PluginEventType::AdapterMessage),
            Arc::new(StaticReplyHandler { reply: "pong" }),
        )
        .with_filter(CommandFilter::new("ping")),
    );

    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_provider_fallback(ProviderFallbackConfig::default().require_wake(true))
            .with_plugin_registry(Arc::new(plugins)),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("/ping", sink.clone()))
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert_eq!(sink.messages().await[0].chain.plain_text(), "pong");
}

#[tokio::test]
async fn process_stage_runs_plugin_generated_provider_request() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let mut plugins = PluginRegistry::new();
    plugins.register_handler(
        RegisteredHandler::new(
            HandlerMetadata::new("builtin", "ask", PluginEventType::AdapterMessage),
            Arc::new(ProviderRequestHandler),
        )
        .with_filter(CommandFilter::new("ask")),
    );

    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_provider_fallback(ProviderFallbackConfig::default().require_wake(true))
            .with_plugin_registry(Arc::new(plugins)),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("/ask ignored fallback", sink.clone()))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].provider_id.as_deref(),
        Some("configured-provider")
    );
    assert_eq!(requests[0].prompt, "plugin prompt");
    assert_eq!(requests[0].session_id, "plugin-session");
    assert_eq!(
        requests[0].image_urls,
        vec!["https://example.test/plugin.png".to_string()]
    );
    assert_eq!(
        requests[0].system_prompt.as_deref(),
        Some("system from plugin")
    );
    assert_eq!(requests[0].model.as_deref(), Some("plugin-model"));
    assert_eq!(requests[0].wake_prefix.as_deref(), Some("llm"));
    assert_eq!(requests[0].contexts.len(), 1);
    assert_eq!(requests[0].extra_user_content_parts.len(), 1);
    assert_eq!(requests[0].tool_placeholders.len(), 1);
    assert_eq!(sink.messages().await[0].chain.plain_text(), "mock-response");
}

#[tokio::test]
async fn process_stage_falls_back_to_provider_when_plugin_does_not_set_result() {
    let provider = Arc::new(CapturingProvider::default());
    let agent_hook = Arc::new(CapturingAgentHook::default());
    let sink = Arc::new(RecordingSink::default());
    let mut plugins = PluginRegistry::new();
    plugins.register_handler(RegisteredHandler::new(
        HandlerMetadata::new("builtin", "observe", PluginEventType::AdapterMessage),
        Arc::new(NoopHandler),
    ));

    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_agent_run_hook(agent_hook.clone())
            .with_plugin_registry(Arc::new(plugins)),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event("hello", sink.clone()))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].prompt, "hello");
    let hook_events = agent_hook.events.lock().await;
    assert_eq!(hook_events.len(), 4);
    assert_eq!(hook_events[0].kind(), AgentHookEventKind::AgentBegin);
    assert_eq!(hook_events[1].kind(), AgentHookEventKind::WaitingLlmRequest);
    assert_eq!(hook_events[2].kind(), AgentHookEventKind::LlmRequest);
    let AgentHookEvent::AgentDone(done) = &hook_events[3] else {
        panic!("provider fallback should finish through typed agent hook");
    };
    assert_eq!(done.lifecycle.session_id, "conversation-1");
    assert_eq!(done.chain.plain_text(), "mock-response");
    assert_eq!(sink.messages().await[0].chain.plain_text(), "mock-response");
}

#[tokio::test]
async fn process_stage_forwards_image_only_messages_to_provider() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(PipelineContext::with_chat_provider(provider.clone()))
        .with_stage(ProcessStage)
        .with_stage(RespondStage);

    scheduler
        .execute(event_with_chain(
            MessageChain::new(vec![MessageComponent::image(
                "https://example.test/image.png",
            )]),
            sink.clone(),
        ))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].prompt, "");
    assert_eq!(
        requests[0].image_urls,
        vec!["https://example.test/image.png".to_string()]
    );
    assert_eq!(sink.messages().await[0].chain.plain_text(), "mock-response");
}
