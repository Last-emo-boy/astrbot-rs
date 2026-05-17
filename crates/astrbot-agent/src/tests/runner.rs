use std::sync::Arc;

use crate::{
    AgentFallbackPolicy, AgentFeedbackEvent, AgentFeedbackEventKind, AgentHookEvent,
    AgentHookEventKind, AgentRunOutcome, AgentRunner, ChatAgentRunner,
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
            AgentHookEventKind::AgentDone
        ]
    );
    let events = hook.events();
    let AgentHookEvent::AgentDone(done) = &events[1] else {
        panic!("second hook should be agent done");
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
    let AgentHookEvent::AgentDone(done) = &events[1] else {
        panic!("second hook should be agent done");
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
