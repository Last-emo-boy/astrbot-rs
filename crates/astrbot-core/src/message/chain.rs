use serde::{Deserialize, Serialize};

use super::component::MessageComponent;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageChain {
    components: Vec<MessageComponent>,
}

impl MessageChain {
    pub fn new(components: Vec<MessageComponent>) -> Self {
        Self { components }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            components: vec![MessageComponent::plain(text)],
        }
    }

    pub fn push(&mut self, component: MessageComponent) {
        self.components.push(component);
    }

    pub fn components(&self) -> &[MessageComponent] {
        &self.components
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty() || self.components.iter().all(MessageComponent::is_empty)
    }

    pub fn has_sendable_content(&self) -> bool {
        self.components
            .iter()
            .any(MessageComponent::has_sendable_content)
    }

    pub fn retain_valid_send_components(&mut self) {
        self.components
            .retain(MessageComponent::is_valid_send_component);
    }

    pub fn into_sendable(mut self) -> Option<Self> {
        self.retain_valid_send_components();
        self.has_sendable_content().then_some(self)
    }

    pub fn plain_text(&self) -> String {
        self.components
            .iter()
            .filter_map(|component| match component {
                MessageComponent::Plain { text } => Some(text.as_str()),
                MessageComponent::Image { .. }
                | MessageComponent::Record { .. }
                | MessageComponent::Video { .. }
                | MessageComponent::File { .. }
                | MessageComponent::Mention { .. }
                | MessageComponent::MentionAll
                | MessageComponent::Reply { .. } => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn image_urls(&self) -> Vec<String> {
        self.components
            .iter()
            .filter_map(|component| match component {
                MessageComponent::Image { url } if !url.trim().is_empty() => {
                    Some(url.trim().to_string())
                }
                _ => None,
            })
            .collect()
    }

    pub fn prefix_first_plain(&mut self, prefix: &str) -> bool {
        if prefix.is_empty() {
            return false;
        }

        for component in &mut self.components {
            if let MessageComponent::Plain { text } = component {
                text.insert_str(0, prefix);
                return true;
            }
        }

        false
    }

    pub fn trim_plain_text_prefix(&mut self, prefix: &str) -> bool {
        if prefix.is_empty() {
            return false;
        }

        for component in &mut self.components {
            let MessageComponent::Plain { text } = component else {
                continue;
            };
            let trimmed = text.trim_start();
            if trimmed.is_empty() {
                continue;
            }
            let Some(stripped) = trimmed.strip_prefix(prefix) else {
                return false;
            };
            *text = stripped.trim_start().to_string();
            return true;
        }

        false
    }

    pub fn mentions_user(&self, user_id: &str) -> bool {
        self.components.iter().any(|component| match component {
            MessageComponent::Mention { user_id: mentioned } => mentioned == user_id,
            _ => false,
        })
    }

    pub fn mentions_all(&self) -> bool {
        self.components
            .iter()
            .any(|component| matches!(component, MessageComponent::MentionAll))
    }

    pub fn replies_to_user(&self, user_id: &str) -> bool {
        self.components.iter().any(|component| match component {
            MessageComponent::Reply {
                sender_id: Some(sender_id),
                ..
            } => sender_id == user_id,
            _ => false,
        })
    }
}

impl From<&str> for MessageChain {
    fn from(value: &str) -> Self {
        Self::plain(value)
    }
}

impl From<String> for MessageChain {
    fn from(value: String) -> Self {
        Self::plain(value)
    }
}
