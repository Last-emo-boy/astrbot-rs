use std::sync::Arc;

use astrbot_core::{
    AstrbotError, EventExecutor, MessageChain, MessageComponent, MessageEvent, MessageEventResult,
    MessageSender, MessageSession, ProviderContentPart, ProviderContextMessage, ProviderRequest,
    ProviderToolPlaceholder, Result,
};
use astrbot_pipeline::{
    InMemoryProviderPreferencePort, PipelineContext, PipelineScheduler, ProviderFallbackConfig,
    SessionContextPort,
    stages::{ProcessStage, RespondStage},
};
use astrbot_platform::RecordingSink;
use astrbot_plugin::{
    CommandFilter, HandlerMetadata, PluginControl, PluginEventType, PluginHandler, PluginRegistry,
    RegisteredHandler,
};
use astrbot_provider::{ChatProvider, ChatRequest, ChatResponse};
use async_trait::async_trait;
use tokio::sync::Mutex;

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

#[tokio::test]
async fn process_stage_falls_back_to_provider_when_plugin_does_not_set_result() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let mut plugins = PluginRegistry::new();
    plugins.register_handler(RegisteredHandler::new(
        HandlerMetadata::new("builtin", "observe", PluginEventType::AdapterMessage),
        Arc::new(NoopHandler),
    ));

    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
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

#[tokio::test]
async fn process_stage_injects_reply_selected_text_into_provider_request() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(PipelineContext::with_chat_provider(provider.clone()))
        .with_stage(ProcessStage)
        .with_stage(RespondStage);

    scheduler
        .execute(event_with_chain(
            MessageChain::new(vec![
                MessageComponent::reply("message-1", "previous answer"),
                MessageComponent::plain("continue"),
            ]),
            sink.clone(),
        ))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].prompt, "continue");
    assert_eq!(
        requests[0].extra_user_content_parts,
        vec![ProviderContentPart::text(
            "<Quoted Message>\nprevious answer\n</Quoted Message>"
        )]
    );
}

#[tokio::test]
async fn process_stage_ignores_blank_reply_selected_text() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(PipelineContext::with_chat_provider(provider.clone()))
        .with_stage(ProcessStage)
        .with_stage(RespondStage);

    scheduler
        .execute(event_with_chain(
            MessageChain::new(vec![
                MessageComponent::reply("message-1", " "),
                MessageComponent::plain("continue"),
            ]),
            sink.clone(),
        ))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert!(requests[0].extra_user_content_parts.is_empty());
}

#[tokio::test]
async fn process_stage_reply_only_message_does_not_trigger_provider_fallback() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(PipelineContext::with_chat_provider(provider.clone()))
        .with_stage(ProcessStage)
        .with_stage(RespondStage);

    scheduler
        .execute(event_with_chain(
            MessageChain::new(vec![MessageComponent::reply(
                "message-1",
                "previous answer",
            )]),
            sink.clone(),
        ))
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn process_stage_adds_quote_context_to_plugin_provider_request() {
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
            .with_plugin_registry(Arc::new(plugins)),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(event_with_chain(
            MessageChain::new(vec![
                MessageComponent::reply("message-1", "previous answer"),
                MessageComponent::plain("/ask ignored fallback"),
            ]),
            sink.clone(),
        ))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].extra_user_content_parts,
        vec![
            ProviderContentPart::text("<Quoted Message>\nprevious answer\n</Quoted Message>"),
            ProviderContentPart::text("extra instruction"),
        ]
    );
}

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

fn direct_event(text: impl Into<String>, sink: Arc<RecordingSink>) -> MessageEvent {
    event_with_chain(MessageChain::plain(text), sink)
}

fn event_with_chain(message: MessageChain, sink: Arc<RecordingSink>) -> MessageEvent {
    MessageEvent::new(
        "event-1",
        "mock",
        "Mock Platform",
        MessageSession::new("mock", "conversation-1"),
        MessageSender::new("user-1", None),
        message,
        sink,
    )
}

#[derive(Default)]
struct CapturingProvider {
    requests: Mutex<Vec<ChatRequest>>,
}

#[async_trait]
impl ChatProvider for CapturingProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        self.requests.lock().await.push(request);
        Ok(ChatResponse::text("mock-response"))
    }
}

struct FailingProvider;

#[async_trait]
impl ChatProvider for FailingProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse> {
        Err(AstrbotError::Provider(
            "upstream returned secret details".to_string(),
        ))
    }
}

struct StaticSessionContextPort {
    contexts: Vec<ProviderContextMessage>,
}

impl StaticSessionContextPort {
    fn new(contexts: Vec<ProviderContextMessage>) -> Self {
        Self { contexts }
    }
}

#[async_trait]
impl SessionContextPort for StaticSessionContextPort {
    async fn context_messages(&self, _event: &MessageEvent) -> Result<Vec<ProviderContextMessage>> {
        Ok(self.contexts.clone())
    }
}

struct StaticReplyHandler {
    reply: &'static str,
}

#[async_trait]
impl PluginHandler for StaticReplyHandler {
    async fn handle(&self, event: &mut MessageEvent) -> Result<PluginControl> {
        event.set_result(MessageEventResult::general(self.reply));
        Ok(PluginControl::Continue)
    }
}

struct NoopHandler;

#[async_trait]
impl PluginHandler for NoopHandler {
    async fn handle(&self, _event: &mut MessageEvent) -> Result<PluginControl> {
        Ok(PluginControl::Continue)
    }
}

struct ProviderRequestHandler;

#[async_trait]
impl PluginHandler for ProviderRequestHandler {
    async fn handle(&self, event: &mut MessageEvent) -> Result<PluginControl> {
        event.set_provider_request(
            ProviderRequest::new("plugin prompt", "plugin-session")
                .with_provider_id("configured-provider")
                .with_image_url("https://example.test/plugin.png")
                .with_system_prompt("system from plugin")
                .with_model("plugin-model")
                .with_wake_prefix("llm")
                .with_context(ProviderContextMessage::text("assistant", "previous"))
                .with_extra_user_content_part(ProviderContentPart::text("extra instruction"))
                .with_tool_placeholder(ProviderToolPlaceholder::new("search")),
        );
        Ok(PluginControl::Continue)
    }
}
