use astrbot_core::MessageChain;

use super::{ProviderReasoningMetadata, ProviderTokenUsage, ProviderToolCall};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderStreamEventKind {
    Delta,
    Reasoning,
    ToolCall,
    Usage,
    Done,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderStreamEvent {
    pub kind: ProviderStreamEventKind,
    pub chain: Option<MessageChain>,
    pub reasoning: Option<ProviderReasoningMetadata>,
    pub tool_call: Option<ProviderToolCall>,
    pub usage: Option<ProviderTokenUsage>,
    pub error_message: Option<String>,
}

impl ProviderStreamEvent {
    pub fn delta(chain: impl Into<MessageChain>) -> Self {
        Self {
            kind: ProviderStreamEventKind::Delta,
            chain: Some(chain.into()),
            reasoning: None,
            tool_call: None,
            usage: None,
            error_message: None,
        }
    }

    pub fn reasoning(reasoning: ProviderReasoningMetadata) -> Self {
        Self {
            kind: ProviderStreamEventKind::Reasoning,
            chain: None,
            reasoning: Some(reasoning),
            tool_call: None,
            usage: None,
            error_message: None,
        }
    }

    pub fn tool_call(tool_call: ProviderToolCall) -> Self {
        Self {
            kind: ProviderStreamEventKind::ToolCall,
            chain: None,
            reasoning: None,
            tool_call: Some(tool_call),
            usage: None,
            error_message: None,
        }
    }

    pub fn usage(usage: ProviderTokenUsage) -> Self {
        Self {
            kind: ProviderStreamEventKind::Usage,
            chain: None,
            reasoning: None,
            tool_call: None,
            usage: Some(usage),
            error_message: None,
        }
    }

    pub fn done() -> Self {
        Self {
            kind: ProviderStreamEventKind::Done,
            chain: None,
            reasoning: None,
            tool_call: None,
            usage: None,
            error_message: None,
        }
    }

    pub fn error(error_message: impl Into<String>) -> Self {
        Self {
            kind: ProviderStreamEventKind::Error,
            chain: None,
            reasoning: None,
            tool_call: None,
            usage: None,
            error_message: Some(error_message.into()),
        }
    }
}
