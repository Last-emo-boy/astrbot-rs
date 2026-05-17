use std::time::Duration;

use astrbot_core::MessageChain;
use serde::{Deserialize, Serialize};

use crate::AgentFeedbackEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentResponseEventKind {
    Delta,
    ToolCall,
    ToolResult,
    Final,
    Stats,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

impl AgentTokenUsage {
    pub fn new(prompt_tokens: u64, completion_tokens: u64) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentResponseStats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<AgentTokenUsage>,
    pub duration_ms: u64,
    pub time_to_first_token_ms: u64,
}

impl AgentResponseStats {
    pub fn with_token_usage(mut self, token_usage: AgentTokenUsage) -> Self {
        self.token_usage = Some(token_usage);
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration_ms = duration.as_millis().try_into().unwrap_or(u64::MAX);
        self
    }

    pub fn with_time_to_first_token(mut self, duration: Duration) -> Self {
        self.time_to_first_token_ms = duration.as_millis().try_into().unwrap_or(u64::MAX);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentResponseEvent {
    pub kind: AgentResponseEventKind,
    pub chain: MessageChain,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<AgentResponseStats>,
}

impl AgentResponseEvent {
    pub fn new(kind: AgentResponseEventKind, chain: impl Into<MessageChain>) -> Self {
        Self {
            kind,
            chain: chain.into(),
            stats: None,
        }
    }

    pub fn delta(chain: impl Into<MessageChain>) -> Self {
        Self::new(AgentResponseEventKind::Delta, chain)
    }

    pub fn final_chain(chain: impl Into<MessageChain>) -> Self {
        Self::new(AgentResponseEventKind::Final, chain)
    }

    pub fn stats(stats: AgentResponseStats) -> Self {
        Self {
            kind: AgentResponseEventKind::Stats,
            chain: MessageChain::default(),
            stats: Some(stats),
        }
    }

    pub fn with_stats(mut self, stats: AgentResponseStats) -> Self {
        self.stats = Some(stats);
        self
    }
}

impl From<AgentFeedbackEvent> for AgentResponseEvent {
    fn from(event: AgentFeedbackEvent) -> Self {
        let kind = match event.kind {
            crate::AgentFeedbackEventKind::ToolCall => AgentResponseEventKind::ToolCall,
            crate::AgentFeedbackEventKind::ToolResult => AgentResponseEventKind::ToolResult,
            crate::AgentFeedbackEventKind::StreamingDelta
            | crate::AgentFeedbackEventKind::StreamingBreak => AgentResponseEventKind::Delta,
            crate::AgentFeedbackEventKind::FinalChain => AgentResponseEventKind::Final,
            crate::AgentFeedbackEventKind::Stats => AgentResponseEventKind::Stats,
            crate::AgentFeedbackEventKind::Aborted | crate::AgentFeedbackEventKind::Error => {
                AgentResponseEventKind::Error
            }
        };
        Self::new(kind, event.chain)
    }
}
