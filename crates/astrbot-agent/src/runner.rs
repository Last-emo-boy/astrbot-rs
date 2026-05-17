use std::sync::Arc;

use astrbot_core::{MessageEvent, MessageEventResult, Result};
use astrbot_provider::{ChatProvider, ChatRequest};
use async_trait::async_trait;

use crate::{
    AgentDoneEvent, AgentFallbackPolicy, AgentFeedbackEvent, AgentHookEvent, AgentLifecycleEvent,
    AgentRunContext, AgentRunHook, NoopAgentRunHook, NoopProviderRequestDecorator,
    ProviderRequestDecorator, ProviderRequestEnvelope,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentRunOutcome {
    result: Option<MessageEventResult>,
    feedback_events: Vec<AgentFeedbackEvent>,
}

impl AgentRunOutcome {
    pub fn continue_without_result() -> Self {
        Self {
            result: None,
            feedback_events: Vec::new(),
        }
    }

    pub fn with_result(result: MessageEventResult) -> Self {
        Self {
            result: Some(result),
            feedback_events: Vec::new(),
        }
    }

    pub fn with_feedback_event(mut self, event: AgentFeedbackEvent) -> Self {
        self.feedback_events.push(event);
        self
    }

    pub fn with_feedback_events<I>(mut self, events: I) -> Self
    where
        I: IntoIterator<Item = AgentFeedbackEvent>,
    {
        self.feedback_events.extend(events);
        self
    }

    pub fn result(&self) -> Option<&MessageEventResult> {
        self.result.as_ref()
    }

    pub fn feedback_events(&self) -> &[AgentFeedbackEvent] {
        &self.feedback_events
    }

    pub fn into_result(self) -> Option<MessageEventResult> {
        self.result
    }

    pub fn into_feedback_events(self) -> Vec<AgentFeedbackEvent> {
        self.feedback_events
    }

    pub fn into_parts(self) -> (Option<MessageEventResult>, Vec<AgentFeedbackEvent>) {
        (self.result, self.feedback_events)
    }
}

#[async_trait]
pub trait AgentRunner: Send + Sync {
    async fn run(&self, event: &MessageEvent) -> Result<AgentRunOutcome>;
}

pub struct ChatAgentRunner {
    provider: Arc<dyn ChatProvider>,
    fallback_policy: AgentFallbackPolicy,
    request_decorator: Arc<dyn ProviderRequestDecorator>,
    hook: Arc<dyn AgentRunHook>,
}

impl ChatAgentRunner {
    pub fn new(provider: Arc<dyn ChatProvider>) -> Self {
        Self {
            provider,
            fallback_policy: AgentFallbackPolicy::default(),
            request_decorator: Arc::new(NoopProviderRequestDecorator),
            hook: Arc::new(NoopAgentRunHook),
        }
    }

    pub fn with_fallback_policy(mut self, fallback_policy: AgentFallbackPolicy) -> Self {
        self.fallback_policy = fallback_policy;
        self
    }

    pub fn with_request_decorator(
        mut self,
        request_decorator: Arc<dyn ProviderRequestDecorator>,
    ) -> Self {
        self.request_decorator = request_decorator;
        self
    }

    pub fn with_hook(mut self, hook: Arc<dyn AgentRunHook>) -> Self {
        self.hook = hook;
        self
    }
}

#[async_trait]
impl AgentRunner for ChatAgentRunner {
    async fn run(&self, event: &MessageEvent) -> Result<AgentRunOutcome> {
        if event.result().is_some() || event.is_stopped() {
            return Ok(AgentRunOutcome::continue_without_result());
        }

        if !self.fallback_policy.enabled {
            return Ok(AgentRunOutcome::continue_without_result());
        }

        let Some(mut envelope) = ProviderRequestEnvelope::from_event(event) else {
            return Ok(AgentRunOutcome::continue_without_result());
        };

        if !envelope.explicit && self.fallback_policy.require_wake && !event.is_at_or_wake_command()
        {
            return Ok(AgentRunOutcome::continue_without_result());
        }

        let run_context = AgentRunContext::from_event(event);
        let lifecycle = AgentLifecycleEvent::from_context(&run_context);
        self.hook
            .on_event(AgentHookEvent::AgentBegin(lifecycle.clone()))
            .await?;

        self.request_decorator
            .decorate(event, &mut envelope.request)
            .await?;

        let response = match self
            .provider
            .chat(ChatRequest::from(envelope.request))
            .await
        {
            Ok(response) => response,
            Err(err) => {
                let Some(message) = self.fallback_policy.error_message.clone() else {
                    return Err(err);
                };
                return Ok(AgentRunOutcome::with_result(MessageEventResult::general(
                    message,
                )));
            }
        };
        let mut done_event = AgentDoneEvent::new(lifecycle, response.chain.clone());
        if let Some(reasoning) = response
            .metadata
            .reasoning
            .as_ref()
            .filter(|reasoning| !reasoning.content.trim().is_empty())
        {
            done_event = done_event.with_reasoning_content(reasoning.content.clone());
        }
        self.hook
            .on_event(AgentHookEvent::AgentDone(done_event))
            .await?;

        Ok(AgentRunOutcome::with_result(MessageEventResult::llm(
            response.chain,
        )))
    }
}
