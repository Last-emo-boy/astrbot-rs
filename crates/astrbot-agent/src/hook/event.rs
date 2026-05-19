use astrbot_core::{MessageChain, MessageEvent, ProviderRequest, ProviderToolCallResult};

use crate::{AgentRunContext, AgentToolCall};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentHookEvent {
    AgentBegin(AgentLifecycleEvent),
    WaitingLlmRequest(AgentLifecycleEvent),
    LlmRequest(AgentLlmRequestEvent),
    ToolStart(AgentToolLifecycleEvent),
    ToolEnd(AgentToolLifecycleEvent),
    AgentDone(AgentDoneEvent),
}

impl AgentHookEvent {
    pub fn kind(&self) -> AgentHookEventKind {
        match self {
            Self::AgentBegin(_) => AgentHookEventKind::AgentBegin,
            Self::WaitingLlmRequest(_) => AgentHookEventKind::WaitingLlmRequest,
            Self::LlmRequest(_) => AgentHookEventKind::LlmRequest,
            Self::ToolStart(_) => AgentHookEventKind::ToolStart,
            Self::ToolEnd(_) => AgentHookEventKind::ToolEnd,
            Self::AgentDone(_) => AgentHookEventKind::AgentDone,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentHookEventKind {
    AgentBegin,
    WaitingLlmRequest,
    LlmRequest,
    ToolStart,
    ToolEnd,
    AgentDone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentLifecycleEvent {
    pub event_id: String,
    pub session_id: String,
}

impl AgentLifecycleEvent {
    pub fn from_context<C>(context: &AgentRunContext<C>) -> Self {
        Self {
            event_id: context.event_id().to_string(),
            session_id: context.session_id().to_string(),
        }
    }

    pub fn from_event(event: &MessageEvent) -> Self {
        Self {
            event_id: event.id.clone(),
            session_id: event.session.conversation_id.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentLlmRequestEvent {
    pub lifecycle: AgentLifecycleEvent,
    pub request: ProviderRequest,
    pub explicit: bool,
}

impl AgentLlmRequestEvent {
    pub fn new(lifecycle: AgentLifecycleEvent, request: ProviderRequest, explicit: bool) -> Self {
        Self {
            lifecycle,
            request,
            explicit,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentToolLifecycleEvent {
    pub lifecycle: AgentLifecycleEvent,
    pub tool_call: AgentToolCall,
    pub result: Option<ProviderToolCallResult>,
}

impl AgentToolLifecycleEvent {
    pub fn start(lifecycle: AgentLifecycleEvent, tool_call: AgentToolCall) -> Self {
        Self {
            lifecycle,
            tool_call,
            result: None,
        }
    }

    pub fn end(
        lifecycle: AgentLifecycleEvent,
        tool_call: AgentToolCall,
        result: ProviderToolCallResult,
    ) -> Self {
        Self {
            lifecycle,
            tool_call,
            result: Some(result),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentDoneEvent {
    pub lifecycle: AgentLifecycleEvent,
    pub chain: MessageChain,
    pub reasoning_content: Option<String>,
}

impl AgentDoneEvent {
    pub fn new(lifecycle: AgentLifecycleEvent, chain: impl Into<MessageChain>) -> Self {
        Self {
            lifecycle,
            chain: chain.into(),
            reasoning_content: None,
        }
    }

    pub fn with_reasoning_content(mut self, reasoning_content: impl Into<String>) -> Self {
        let reasoning_content = reasoning_content.into();
        self.reasoning_content =
            (!reasoning_content.trim().is_empty()).then_some(reasoning_content);
        self
    }
}
