use astrbot_core::ProviderContextMessage;

use super::AgentTokenCounter;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentContextWindow {
    messages: Vec<ProviderContextMessage>,
}

impl AgentContextWindow {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_messages(messages: Vec<ProviderContextMessage>) -> Self {
        Self { messages }
    }

    pub fn messages(&self) -> &[ProviderContextMessage] {
        &self.messages
    }

    pub fn into_messages(self) -> Vec<ProviderContextMessage> {
        self.messages
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn total_tokens(&self, counter: &dyn AgentTokenCounter) -> usize {
        self.messages
            .iter()
            .map(|message| counter.count_message(message))
            .sum()
    }
}

impl From<Vec<ProviderContextMessage>> for AgentContextWindow {
    fn from(messages: Vec<ProviderContextMessage>) -> Self {
        Self::from_messages(messages)
    }
}
