use std::sync::{Arc, Mutex};

use astrbot_core::{MessageEvent, ProviderRequest, Result};
use async_trait::async_trait;

use crate::{
    AgentFallbackPolicy, AgentFeedbackEvent, AgentFeedbackEventKind, AgentHookEvent,
    AgentHookEventKind, AgentRunOutcome, AgentRunner, ChatAgentRunner, ProviderRequestHook,
};

use super::support::{CapturingHook, CapturingProvider, event};

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
async fn chat_agent_runner_dispatches_begin_and_done_hooks_around_provider_call() {
    let hook = Arc::new(CapturingHook::default());
    let runner =
        ChatAgentRunner::new(Arc::new(CapturingProvider::default())).with_hook(hook.clone());

    let outcome = runner.run(&event("hello")).await.expect("agent should run");

    assert_eq!(
        outcome.result().expect("result").chain.plain_text(),
        "conversation-1:hello"
    );
    assert_eq!(
        hook.kinds(),
        vec![
            AgentHookEventKind::AgentBegin,
            AgentHookEventKind::WaitingLlmRequest,
            AgentHookEventKind::LlmRequest,
            AgentHookEventKind::AgentDone
        ]
    );
    let events = hook.events();
    let AgentHookEvent::LlmRequest(request_event) = &events[2] else {
        panic!("third hook should be LLM request");
    };
    assert_eq!(request_event.lifecycle.event_id, "event-1");
    assert_eq!(request_event.request.prompt.as_deref(), Some("hello"));
    assert!(!request_event.explicit);
    let AgentHookEvent::AgentDone(done) = &events[3] else {
        panic!("fourth hook should be agent done");
    };
    assert_eq!(done.lifecycle.event_id, "event-1");
    assert_eq!(done.chain.plain_text(), "conversation-1:hello");
}

#[tokio::test]
async fn chat_agent_runner_forwards_provider_reasoning_to_agent_done_hook() {
    let hook = Arc::new(CapturingHook::default());
    let runner = ChatAgentRunner::new(Arc::new(CapturingProvider {
        fail: false,
        reasoning_content: Some("hidden reasoning".to_string()),
    }))
    .with_hook(hook.clone());

    runner.run(&event("hello")).await.expect("agent should run");

    let events = hook.events();
    let AgentHookEvent::AgentDone(done) = &events[3] else {
        panic!("fourth hook should be agent done");
    };
    assert_eq!(done.reasoning_content.as_deref(), Some("hidden reasoning"));
}

#[tokio::test]
async fn chat_agent_runner_maps_provider_error_to_fallback_message() {
    let runner = ChatAgentRunner::new(Arc::new(CapturingProvider {
        fail: true,
        reasoning_content: None,
    }))
    .with_fallback_policy(AgentFallbackPolicy::default().with_error_message("try later"));

    let outcome = runner
        .run(&event("hello"))
        .await
        .expect("fallback message should be returned");

    let result = outcome.result().expect("fallback should set result");
    assert_eq!(result.chain.plain_text(), "try later");
}

#[tokio::test]
async fn chat_agent_runner_strips_matching_provider_wake_prefix_before_provider_call() {
    let runner = ChatAgentRunner::new(Arc::new(CapturingProvider::default()))
        .with_fallback_policy(AgentFallbackPolicy::default().with_provider_wake_prefix("llm"));

    let outcome = runner
        .run(&event("llm explain"))
        .await
        .expect("agent should run");

    assert_eq!(
        outcome.result().expect("result").chain.plain_text(),
        "conversation-1:explain"
    );
}

#[tokio::test]
async fn chat_agent_runner_skips_implicit_request_when_provider_wake_prefix_is_missing() {
    let runner = ChatAgentRunner::new(Arc::new(CapturingProvider::default()))
        .with_fallback_policy(AgentFallbackPolicy::default().with_provider_wake_prefix("llm"));

    let outcome = runner
        .run(&event("plain prompt"))
        .await
        .expect("agent should run");

    assert!(outcome.result().is_none());
}

#[tokio::test]
async fn chat_agent_runner_request_hook_can_modify_request_before_provider_call() {
    let runner = ChatAgentRunner::new(Arc::new(CapturingProvider::default()))
        .with_request_hook(Arc::new(RewriteRequestHook::default()));

    let outcome = runner.run(&event("hello")).await.expect("agent should run");

    assert_eq!(
        outcome.result().expect("result").chain.plain_text(),
        "hook-session:rewritten by hook"
    );
}

#[tokio::test]
async fn chat_agent_runner_request_hook_can_stop_before_provider_call() {
    let hook = Arc::new(RewriteRequestHook {
        stop: true,
        ..RewriteRequestHook::default()
    });
    let runner = ChatAgentRunner::new(Arc::new(CapturingProvider::default()))
        .with_request_hook(hook.clone());

    let outcome = runner.run(&event("hello")).await.expect("agent should run");

    assert!(outcome.result().is_none());
    assert_eq!(hook.seen_prompts(), vec!["hello".to_string()]);
}

#[derive(Default)]
struct RewriteRequestHook {
    stop: bool,
    seen: Mutex<Vec<String>>,
}

impl RewriteRequestHook {
    fn seen_prompts(&self) -> Vec<String> {
        self.seen.lock().expect("seen prompts lock").clone()
    }
}

#[async_trait]
impl ProviderRequestHook for RewriteRequestHook {
    async fn before_request(
        &self,
        _event: &MessageEvent,
        request: &mut ProviderRequest,
        _explicit: bool,
    ) -> Result<bool> {
        self.seen
            .lock()
            .expect("seen prompts lock")
            .push(request.prompt.clone().unwrap_or_default());
        if !self.stop {
            request.prompt = Some("rewritten by hook".to_string());
            request.session_id = Some("hook-session".to_string());
        }
        Ok(self.stop)
    }
}
