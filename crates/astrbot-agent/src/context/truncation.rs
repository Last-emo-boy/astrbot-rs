use astrbot_core::ProviderContextMessage;

use super::{AgentContextWindow, AgentTokenCounter, ContextTokenBudget};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextTruncationPolicy {
    keep_recent_messages: Option<usize>,
    preserve_leading_system_messages: bool,
}

impl ContextTruncationPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn keep_recent_messages(mut self, keep_recent_messages: usize) -> Self {
        self.keep_recent_messages = Some(keep_recent_messages);
        self
    }

    pub fn preserve_leading_system_messages(mut self, preserve: bool) -> Self {
        self.preserve_leading_system_messages = preserve;
        self
    }

    pub fn truncate_by_message_count(&self, window: AgentContextWindow) -> AgentContextWindow {
        let Some(keep_recent_messages) = self.keep_recent_messages else {
            return window;
        };

        let (mut system_messages, non_system_messages) =
            self.split_leading_system_messages(window.into_messages());

        if non_system_messages.len() <= keep_recent_messages {
            system_messages.extend(non_system_messages);
            return AgentContextWindow::from_messages(system_messages);
        }

        let recent_start = non_system_messages.len() - keep_recent_messages;
        let recent_messages = self.trim_to_first_user(non_system_messages[recent_start..].to_vec());
        system_messages.extend(recent_messages);
        AgentContextWindow::from_messages(system_messages)
    }

    pub fn truncate_to_token_budget(
        &self,
        window: AgentContextWindow,
        budget: &ContextTokenBudget,
        counter: &dyn AgentTokenCounter,
    ) -> AgentContextWindow {
        let Some(max_tokens) = budget.available_context_tokens() else {
            return window;
        };

        let (mut system_messages, non_system_messages) =
            self.split_leading_system_messages(window.into_messages());
        let mut used_tokens = system_messages
            .iter()
            .map(|message| counter.count_message(message))
            .sum::<usize>();
        let mut selected = Vec::new();

        for message in non_system_messages.iter().rev() {
            let message_tokens = counter.count_message(message);
            if used_tokens + message_tokens <= max_tokens {
                used_tokens += message_tokens;
                selected.push(message.clone());
            }
        }

        selected.reverse();
        system_messages.extend(self.trim_to_first_user(selected));
        AgentContextWindow::from_messages(system_messages)
    }

    fn split_leading_system_messages(
        &self,
        messages: Vec<ProviderContextMessage>,
    ) -> (Vec<ProviderContextMessage>, Vec<ProviderContextMessage>) {
        if !self.preserve_leading_system_messages {
            return (Vec::new(), messages);
        }

        let split_at = messages
            .iter()
            .position(|message| message.role != "system")
            .unwrap_or(messages.len());

        let mut system_messages = messages;
        let non_system_messages = system_messages.split_off(split_at);
        (system_messages, non_system_messages)
    }

    fn trim_to_first_user(
        &self,
        messages: Vec<ProviderContextMessage>,
    ) -> Vec<ProviderContextMessage> {
        let Some(first_user_index) = messages.iter().position(|message| message.role == "user")
        else {
            return messages;
        };

        if first_user_index == 0 {
            messages
        } else {
            messages[first_user_index..].to_vec()
        }
    }
}

impl Default for ContextTruncationPolicy {
    fn default() -> Self {
        Self {
            keep_recent_messages: None,
            preserve_leading_system_messages: true,
        }
    }
}
