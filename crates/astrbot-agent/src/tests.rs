use std::sync::Arc;

use astrbot_core::{
    AstrbotError, MessageChain, MessageEvent, MessageSender, MessageSession, MessageSink,
    ProviderContentPart, ProviderContextMessage, ProviderRequest, Result,
};
use astrbot_provider::{ChatProvider, ChatRequest, ChatResponse};
use async_trait::async_trait;

use crate::{
    AgentContextCompressor, AgentContextWindow, AgentFallbackPolicy, AgentPersona,
    AgentProviderPreferencePort, AgentQuoteContextPort, AgentRunner, AgentSessionContextPort,
    AgentTokenCounter, ApproximateTokenCounter, ChatAgentRunner, CompositeProviderRequestDecorator,
    ContextTokenBudget, ContextTruncationPolicy, ContextWindowManager,
    ContextWindowRequestDecorator, NoopContextCompressor, PersonaPromptDecorator,
    ProviderPreferenceRequestDecorator, ProviderRequestDecorator, QuoteContextRequestDecorator,
    SessionContextRequestDecorator, ToolLoopPolicy,
};

struct NoopSink;

#[async_trait]
impl MessageSink for NoopSink {
    async fn send(&self, _session: &MessageSession, _chain: MessageChain) -> Result<()> {
        Ok(())
    }
}

fn event(text: impl Into<String>) -> MessageEvent {
    MessageEvent::new(
        "event-1",
        "webchat",
        "WebChat",
        MessageSession::new("webchat", "conversation-1"),
        MessageSender::new("user-1", None),
        MessageChain::plain(text),
        Arc::new(NoopSink),
    )
}

struct StaticPreference;

#[async_trait]
impl AgentProviderPreferencePort for StaticPreference {
    async fn preferred_chat_provider_id(&self, _event: &MessageEvent) -> Result<Option<String>> {
        Ok(Some("preferred-provider".to_string()))
    }
}

struct StaticSessionContext;

#[async_trait]
impl AgentSessionContextPort for StaticSessionContext {
    async fn context_messages(&self, _event: &MessageEvent) -> Result<Vec<ProviderContextMessage>> {
        Ok(vec![ProviderContextMessage::text("assistant", "previous")])
    }
}

struct StaticQuoteContext;

#[async_trait]
impl AgentQuoteContextPort for StaticQuoteContext {
    async fn quote_content_parts(&self, _event: &MessageEvent) -> Result<Vec<ProviderContentPart>> {
        Ok(vec![ProviderContentPart::text("quoted")])
    }
}

struct OneTokenPerMessageCounter;

impl AgentTokenCounter for OneTokenPerMessageCounter {
    fn count_text(&self, _text: &str) -> usize {
        1
    }

    fn count_message(&self, _message: &ProviderContextMessage) -> usize {
        1
    }
}

#[tokio::test]
async fn composite_decorator_applies_preference_context_quote_and_persona() {
    let decorator = CompositeProviderRequestDecorator::new()
        .with_decorator(Arc::new(ProviderPreferenceRequestDecorator::new(Arc::new(
            StaticPreference,
        ))))
        .with_decorator(Arc::new(SessionContextRequestDecorator::new(Arc::new(
            StaticSessionContext,
        ))))
        .with_decorator(Arc::new(QuoteContextRequestDecorator::new(Arc::new(
            StaticQuoteContext,
        ))))
        .with_decorator(Arc::new(PersonaPromptDecorator::new(
            AgentPersona::new("default").with_system_prompt("persona prompt"),
        )));
    let mut request = ProviderRequest::new("hello", "conversation-1");

    decorator
        .decorate(&event("hello"), &mut request)
        .await
        .expect("request should decorate");

    assert_eq!(request.provider_id.as_deref(), Some("preferred-provider"));
    assert_eq!(request.contexts.len(), 1);
    assert_eq!(request.contexts[0].role, "assistant");
    assert_eq!(
        request.extra_user_content_parts,
        vec![ProviderContentPart::text("quoted")]
    );
    assert_eq!(request.system_prompt.as_deref(), Some("persona prompt"));
}

#[test]
fn approximate_counter_counts_text_parts_across_context_window() {
    let counter = ApproximateTokenCounter;
    let window = AgentContextWindow::from_messages(vec![
        ProviderContextMessage::text("user", "hello"),
        ProviderContextMessage::text("assistant", "你好"),
    ]);

    assert_eq!(window.total_tokens(&counter), 4);
}

#[tokio::test]
async fn context_window_manager_truncates_to_newest_messages_under_budget() {
    let manager = ContextWindowManager::new(ContextTokenBudget::new(2))
        .with_token_counter(Arc::new(OneTokenPerMessageCounter))
        .with_truncation_policy(ContextTruncationPolicy::new());
    let messages = vec![
        ProviderContextMessage::text("user", "old user"),
        ProviderContextMessage::text("assistant", "old assistant"),
        ProviderContextMessage::text("user", "new user"),
        ProviderContextMessage::text("assistant", "new assistant"),
    ];

    let prepared = manager
        .prepare_messages(messages)
        .await
        .expect("context should truncate");

    assert_eq!(
        prepared,
        vec![
            ProviderContextMessage::text("user", "new user"),
            ProviderContextMessage::text("assistant", "new assistant"),
        ]
    );
}

#[tokio::test]
async fn context_window_decorator_rewrites_only_contexts() {
    let manager = Arc::new(
        ContextWindowManager::new(ContextTokenBudget::new(1))
            .with_token_counter(Arc::new(OneTokenPerMessageCounter)),
    );
    let decorator = ContextWindowRequestDecorator::new(manager);
    let mut request = ProviderRequest::new("hello", "conversation-1")
        .with_provider_id("provider-1")
        .with_extra_user_content_part(ProviderContentPart::text("quoted"));
    request.contexts = vec![
        ProviderContextMessage::text("user", "old user"),
        ProviderContextMessage::text("assistant", "old assistant"),
        ProviderContextMessage::text("user", "new user"),
    ];

    decorator
        .decorate(&event("hello"), &mut request)
        .await
        .expect("context decorator should run");

    assert_eq!(request.prompt.as_deref(), Some("hello"));
    assert_eq!(request.provider_id.as_deref(), Some("provider-1"));
    assert_eq!(
        request.extra_user_content_parts,
        vec![ProviderContentPart::text("quoted")]
    );
    assert_eq!(
        request.contexts,
        vec![ProviderContextMessage::text("user", "new user")]
    );
}

#[tokio::test]
async fn noop_context_compressor_keeps_window_shape() {
    let compressor = NoopContextCompressor;
    let counter = ApproximateTokenCounter;
    let budget = ContextTokenBudget::new(1);
    let window = AgentContextWindow::from_messages(vec![
        ProviderContextMessage::text("user", "hello"),
        ProviderContextMessage::text("assistant", "world"),
    ]);

    let compressed = compressor
        .compress(window.clone(), &budget, &counter)
        .await
        .expect("noop compressor should succeed");

    assert_eq!(compressed, window);
}

#[derive(Default)]
struct CapturingProvider {
    fail: bool,
}

#[async_trait]
impl ChatProvider for CapturingProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        if self.fail {
            return Err(AstrbotError::Provider("upstream failed".to_string()));
        }

        Ok(ChatResponse::text(format!(
            "{}:{}",
            request.session_id, request.prompt
        )))
    }
}

#[tokio::test]
async fn chat_agent_runner_returns_llm_result_from_provider_response() {
    let runner = ChatAgentRunner::new(Arc::new(CapturingProvider::default()));

    let outcome = runner.run(&event("hello")).await.expect("agent should run");

    let result = outcome
        .result()
        .expect("provider response should set result");
    assert_eq!(result.chain.plain_text(), "conversation-1:hello");
}

#[tokio::test]
async fn chat_agent_runner_maps_provider_error_to_fallback_message() {
    let runner = ChatAgentRunner::new(Arc::new(CapturingProvider { fail: true }))
        .with_fallback_policy(AgentFallbackPolicy::default().with_error_message("try later"));

    let outcome = runner
        .run(&event("hello"))
        .await
        .expect("fallback message should be returned");

    let result = outcome.result().expect("fallback should set result");
    assert_eq!(result.chain.plain_text(), "try later");
}

#[test]
fn tool_loop_policy_normalizes_limits() {
    let policy = ToolLoopPolicy::default()
        .enabled()
        .with_max_steps(0)
        .with_timeout_seconds(0)
        .with_schema_mode("skills-like");

    assert_eq!(policy.max_steps, 1);
    assert_eq!(policy.tool_call_timeout_seconds, 1);
    assert_eq!(policy.schema_mode, "skills-like");
}
