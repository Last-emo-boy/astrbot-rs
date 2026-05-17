use std::time::Duration;

use astrbot_core::MessageEvent;

use crate::AgentMessage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRunContext<C = ()> {
    context: C,
    event_id: String,
    session_id: String,
    messages: Vec<AgentMessage>,
    tool_call_timeout: Duration,
}

impl<C> AgentRunContext<C> {
    pub fn new(context: C, event: &MessageEvent) -> Self {
        Self {
            context,
            event_id: event.id.clone(),
            session_id: event.session.conversation_id.clone(),
            messages: Vec::new(),
            tool_call_timeout: Duration::from_secs(60),
        }
    }

    pub fn with_messages(mut self, messages: Vec<AgentMessage>) -> Self {
        self.messages = messages;
        self
    }

    pub fn with_tool_call_timeout(mut self, tool_call_timeout: Duration) -> Self {
        self.tool_call_timeout = tool_call_timeout.max(Duration::from_secs(1));
        self
    }

    pub fn context(&self) -> &C {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut C {
        &mut self.context
    }

    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn messages(&self) -> &[AgentMessage] {
        &self.messages
    }

    pub fn messages_mut(&mut self) -> &mut Vec<AgentMessage> {
        &mut self.messages
    }

    pub fn push_message(&mut self, message: AgentMessage) {
        self.messages.push(message);
    }

    pub fn tool_call_timeout(&self) -> Duration {
        self.tool_call_timeout
    }
}

impl AgentRunContext<()> {
    pub fn from_event(event: &MessageEvent) -> Self {
        Self::new((), event)
    }
}
