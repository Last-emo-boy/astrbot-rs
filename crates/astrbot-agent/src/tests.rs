use std::sync::Arc;

use astrbot_core::{
    AstrbotError, MessageChain, MessageEvent, MessageSender, MessageSession, MessageSink,
    ProviderContentPart, ProviderContextMessage, ProviderRequest, Result,
};
use astrbot_provider::{ChatProvider, ChatRequest, ChatResponse};
use astrbot_skill::{
    SkillActivationPolicy, SkillCatalog, SkillDescriptor, SkillPromptInventory, SkillSource,
};
use async_trait::async_trait;

use crate::{
    AgentActiveReplyDecider, AgentContextCompressor, AgentContextWindow, AgentFallbackPolicy,
    AgentFeedbackEvent, AgentFeedbackEventKind, AgentMemoryContextPort, AgentPersona,
    AgentProviderPreferencePort, AgentQuoteContextPort, AgentRunOutcome, AgentRunner,
    AgentSessionContextPort, AgentTokenCounter, ApproximateTokenCounter, ChatAgentRunner,
    CompositeProviderRequestDecorator, ContextTokenBudget, ContextTruncationPolicy,
    ContextWindowManager, ContextWindowRequestDecorator, MemoryRequestDecorator,
    NoopContextCompressor, PersonaPromptDecorator, ProviderPreferenceRequestDecorator,
    ProviderRequestDecorator, QuoteContextRequestDecorator, SessionContextRequestDecorator,
    SkillPromptInventoryRequestDecorator, ToolLoopPolicy,
};
use astrbot_memory::{ActiveReplyPolicy, MemorySessionKey, MemoryTranscriptRecord};

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

fn group_event(text: impl Into<String>) -> MessageEvent {
    MessageEvent::new(
        "event-1",
        "webchat",
        "WebChat",
        MessageSession::group("webchat", "room-1"),
        MessageSender::new("user-1", Some("Alice".to_string())),
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

struct StaticMemoryContext;

#[async_trait]
impl AgentMemoryContextPort for StaticMemoryContext {
    async fn memory_records(&self, event: &MessageEvent) -> Result<Vec<MemoryTranscriptRecord>> {
        Ok(vec![MemoryTranscriptRecord::new(
            MemorySessionKey::from_session(&event.session),
            "Alice",
            "[Alice/12:00:00]: hello",
        )])
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

#[tokio::test]
async fn skill_prompt_inventory_decorator_appends_active_skill_prompt_without_package_logic() {
    let mut catalog = SkillCatalog::new();
    catalog.add_skill(
        SkillDescriptor::new("writer", "C:\\skills\\writer\\SKILL.md")
            .with_description("Draft clean text"),
    );
    catalog.add_skill(
        SkillDescriptor::new("preset", "/workspace/skills/preset/SKILL.md")
            .with_description("Sandbox preset")
            .with_source(SkillSource::Sandbox),
    );
    let inventory = SkillPromptInventory::from_catalog(
        &catalog,
        &SkillActivationPolicy::all_enabled().disable("preset"),
    );
    let persona = AgentPersona::new("default")
        .with_system_prompt("persona prompt")
        .with_skills(Some(vec!["writer".to_string()]));
    let decorator = CompositeProviderRequestDecorator::new()
        .with_decorator(Arc::new(PersonaPromptDecorator::new(persona.clone())))
        .with_decorator(Arc::new(SkillPromptInventoryRequestDecorator::for_persona(
            inventory, &persona,
        )));
    let mut request = ProviderRequest::new("hello", "conversation-1");

    decorator
        .decorate(&event("hello"), &mut request)
        .await
        .expect("request should decorate with skill inventory");

    let system_prompt = request.system_prompt.expect("system prompt should exist");
    assert!(system_prompt.contains("persona prompt"));
    assert!(system_prompt.contains("## Skills"));
    assert!(system_prompt.contains("**writer**"));
    assert!(!system_prompt.contains("**preset**"));
    assert!(system_prompt.contains("C:/skills/writer/SKILL.md"));
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
async fn memory_request_decorator_appends_history_to_system_prompt() {
    let decorator = MemoryRequestDecorator::new(Arc::new(StaticMemoryContext));
    let mut request = ProviderRequest::new("what happened", "room-1")
        .with_system_prompt("persona")
        .with_context(ProviderContextMessage::text("assistant", "old"));

    decorator
        .decorate(&group_event("what happened"), &mut request)
        .await
        .expect("memory decorator should run");

    let system_prompt = request
        .system_prompt
        .as_deref()
        .expect("system prompt should exist");
    assert!(system_prompt.contains("persona"));
    assert!(system_prompt.contains("[Alice/12:00:00]: hello"));
    assert_eq!(request.contexts.len(), 1);
}

#[tokio::test]
async fn memory_request_decorator_can_rewrite_active_reply_prompt() {
    let decorator = MemoryRequestDecorator::new(Arc::new(StaticMemoryContext)).active_reply();
    let mut request = ProviderRequest::new("new message", "room-1")
        .with_context(ProviderContextMessage::text("assistant", "old"));

    decorator
        .decorate(&group_event("new message"), &mut request)
        .await
        .expect("memory decorator should run");

    assert!(request.contexts.is_empty());
    let prompt = request.prompt.as_deref().expect("prompt should exist");
    assert!(prompt.contains("[Alice/12:00:00]: hello"));
    assert!(prompt.contains("new message"));
}

#[test]
fn active_reply_decider_uses_memory_policy_without_platform_adapter() {
    let decider = AgentActiveReplyDecider::new(
        ActiveReplyPolicy::probability(0.5).with_whitelist(["room-1"]),
    );

    assert!(decider.should_reply(&group_event("hello"), 0.25));
    assert!(!decider.should_reply(&group_event("hello"), 0.75));

    let mut wake_event = group_event("hello");
    wake_event.mark_wake(true);
    assert!(!decider.should_reply(&wake_event, 0.25));
    assert!(!decider.should_reply(&event("direct"), 0.25));
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

#[test]
fn agent_run_outcome_can_carry_feedback_without_final_result() {
    let outcome = AgentRunOutcome::continue_without_result()
        .with_feedback_event(AgentFeedbackEvent::streaming_delta("partial"))
        .with_feedback_events([AgentFeedbackEvent::tool_call("Calling tool: search")]);

    assert!(outcome.result().is_none());
    assert_eq!(outcome.feedback_events().len(), 2);
    assert_eq!(
        outcome.feedback_events()[0].kind,
        AgentFeedbackEventKind::StreamingDelta
    );

    let (result, feedback_events) = outcome.into_parts();
    assert!(result.is_none());
    assert_eq!(feedback_events.len(), 2);
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
