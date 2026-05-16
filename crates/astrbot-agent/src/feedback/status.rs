use std::collections::HashMap;

use astrbot_core::MessageChain;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentFeedbackEventKind {
    ToolCall,
    ToolResult,
    StreamingDelta,
    StreamingBreak,
    FinalChain,
    Aborted,
    Error,
    Stats,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentFeedbackEvent {
    pub kind: AgentFeedbackEventKind,
    pub chain: MessageChain,
}

impl AgentFeedbackEvent {
    pub fn new(kind: AgentFeedbackEventKind, chain: impl Into<MessageChain>) -> Self {
        Self {
            kind,
            chain: chain.into(),
        }
    }

    pub fn tool_call(message: impl Into<String>) -> Self {
        Self::new(
            AgentFeedbackEventKind::ToolCall,
            MessageChain::plain(message),
        )
    }

    pub fn tool_result(message: impl Into<String>) -> Self {
        Self::new(
            AgentFeedbackEventKind::ToolResult,
            MessageChain::plain(message),
        )
    }

    pub fn streaming_delta(chain: impl Into<MessageChain>) -> Self {
        Self::new(AgentFeedbackEventKind::StreamingDelta, chain)
    }

    pub fn streaming_break() -> Self {
        Self::new(
            AgentFeedbackEventKind::StreamingBreak,
            MessageChain::default(),
        )
    }

    pub fn final_chain(chain: impl Into<MessageChain>) -> Self {
        Self::new(AgentFeedbackEventKind::FinalChain, chain)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentStreamingFeedbackPolicy {
    pub stream_to_general: bool,
    pub emit_break_before_tool_call: bool,
}

impl Default for AgentStreamingFeedbackPolicy {
    fn default() -> Self {
        Self {
            stream_to_general: false,
            emit_break_before_tool_call: true,
        }
    }
}

impl AgentStreamingFeedbackPolicy {
    pub fn stream_to_general(mut self, stream_to_general: bool) -> Self {
        self.stream_to_general = stream_to_general;
        self
    }

    pub fn emit_break_before_tool_call(mut self, emit_break_before_tool_call: bool) -> Self {
        self.emit_break_before_tool_call = emit_break_before_tool_call;
        self
    }

    pub fn streaming_delta_event(
        &self,
        chain: impl Into<MessageChain>,
    ) -> Option<AgentFeedbackEvent> {
        let chain = chain.into();
        if chain.is_empty() {
            return None;
        }

        if self.stream_to_general {
            return self
                .final_chain_from_delta(chain)
                .map(AgentFeedbackEvent::final_chain);
        }

        Some(AgentFeedbackEvent::streaming_delta(chain))
    }

    pub fn final_chain_from_delta(&self, chain: impl Into<MessageChain>) -> Option<MessageChain> {
        chain.into().into_sendable()
    }

    pub fn tool_call_break_event(&self) -> Option<AgentFeedbackEvent> {
        self.emit_break_before_tool_call
            .then(AgentFeedbackEvent::streaming_break)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCallStatus {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: Option<String>,
}

impl ToolCallStatus {
    pub fn new(call_id: impl Into<String>, tool_name: impl Into<String>) -> Self {
        Self {
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            arguments: None,
        }
    }

    pub fn with_arguments(mut self, arguments: impl Into<String>) -> Self {
        self.arguments = non_empty_option(arguments);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolResultStatus {
    pub call_id: Option<String>,
    pub tool_name: Option<String>,
    pub result_text: String,
}

impl ToolResultStatus {
    pub fn new(result_text: impl Into<String>) -> Self {
        Self {
            call_id: None,
            tool_name: None,
            result_text: result_text.into(),
        }
    }

    pub fn with_call_id(mut self, call_id: impl Into<String>) -> Self {
        self.call_id = non_empty_option(call_id);
        self
    }

    pub fn with_tool_name(mut self, tool_name: impl Into<String>) -> Self {
        self.tool_name = non_empty_option(tool_name);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolStatusMessagePolicy {
    pub show_tool_use: bool,
    pub show_tool_call_result: bool,
    pub result_preview_limit: usize,
}

impl Default for ToolStatusMessagePolicy {
    fn default() -> Self {
        Self {
            show_tool_use: true,
            show_tool_call_result: false,
            result_preview_limit: 70,
        }
    }
}

impl ToolStatusMessagePolicy {
    pub fn show_tool_use(mut self, show_tool_use: bool) -> Self {
        self.show_tool_use = show_tool_use;
        self
    }

    pub fn show_tool_call_result(mut self, show_tool_call_result: bool) -> Self {
        self.show_tool_call_result = show_tool_call_result;
        self
    }

    pub fn with_result_preview_limit(mut self, result_preview_limit: usize) -> Self {
        self.result_preview_limit = result_preview_limit.max(1);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolStatusTracker {
    policy: ToolStatusMessagePolicy,
    tool_name_by_call_id: HashMap<String, String>,
}

impl ToolStatusTracker {
    pub fn new(policy: ToolStatusMessagePolicy) -> Self {
        Self {
            policy,
            tool_name_by_call_id: HashMap::new(),
        }
    }

    pub fn record_tool_call(&mut self, status: ToolCallStatus) -> Option<AgentFeedbackEvent> {
        let tool_name = normalized_tool_name(&status.tool_name);
        if let Some(call_id) = non_empty_option(status.call_id) {
            self.tool_name_by_call_id
                .insert(call_id, tool_name.to_string());
        }

        if !self.policy.show_tool_use || self.policy.show_tool_call_result {
            return None;
        }

        Some(AgentFeedbackEvent::tool_call(format!(
            "Calling tool: {tool_name}"
        )))
    }

    pub fn record_tool_result(&mut self, status: ToolResultStatus) -> Option<AgentFeedbackEvent> {
        if !self.policy.show_tool_use || !self.policy.show_tool_call_result {
            return None;
        }

        let tool_name = status
            .tool_name
            .filter(|name| !name.trim().is_empty())
            .or_else(|| {
                status
                    .call_id
                    .and_then(|call_id| self.tool_name_by_call_id.remove(call_id.trim()))
            })
            .unwrap_or_else(|| "unknown".to_string());
        let preview = truncate_preview(&status.result_text, self.policy.result_preview_limit);
        let message = if preview.is_empty() {
            format!("Calling tool: {}", normalized_tool_name(&tool_name))
        } else {
            format!(
                "Calling tool: {}\nResult: {preview}",
                normalized_tool_name(&tool_name)
            )
        };

        Some(AgentFeedbackEvent::tool_result(message))
    }

    pub fn pending_tool_count(&self) -> usize {
        self.tool_name_by_call_id.len()
    }
}

fn normalized_tool_name(tool_name: &str) -> &str {
    let trimmed = tool_name.trim();
    if trimmed.is_empty() {
        "unknown"
    } else {
        trimmed
    }
}

fn truncate_preview(text: &str, limit: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= limit {
        return trimmed.to_string();
    }

    let mut preview = trimmed.chars().take(limit).collect::<String>();
    preview.push_str("...");
    preview
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use astrbot_core::{MessageChain, MessageComponent};

    use super::{
        AgentFeedbackEvent, AgentFeedbackEventKind, AgentStreamingFeedbackPolicy, ToolCallStatus,
        ToolResultStatus, ToolStatusMessagePolicy, ToolStatusTracker,
    };

    #[test]
    fn tool_status_tracker_builds_call_and_result_messages_outside_stream_parser() {
        let mut tracker = ToolStatusTracker::new(
            ToolStatusMessagePolicy::default()
                .show_tool_call_result(true)
                .with_result_preview_limit(5),
        );

        assert_eq!(
            tracker.record_tool_call(ToolCallStatus::new("call-1", "search")),
            None
        );
        let result = tracker
            .record_tool_result(
                ToolResultStatus::new("abcdefghi")
                    .with_call_id("call-1")
                    .with_tool_name(""),
            )
            .expect("tool result status should be emitted");

        assert_eq!(result.kind, AgentFeedbackEventKind::ToolResult);
        assert_eq!(
            result.chain.plain_text(),
            "Calling tool: search\nResult: abcde..."
        );
        assert_eq!(tracker.pending_tool_count(), 0);
    }

    #[test]
    fn tool_status_policy_can_disable_user_facing_messages() {
        let mut tracker =
            ToolStatusTracker::new(ToolStatusMessagePolicy::default().show_tool_use(false));

        assert_eq!(
            tracker.record_tool_call(ToolCallStatus::new("call-1", "search")),
            None
        );
        assert_eq!(
            tracker.record_tool_result(ToolResultStatus::new("ok").with_call_id("call-1")),
            None
        );
    }

    #[test]
    fn streaming_feedback_event_keeps_chunk_without_status_text() {
        let event = AgentFeedbackEvent::streaming_delta("hello");

        assert_eq!(event.kind, AgentFeedbackEventKind::StreamingDelta);
        assert_eq!(event.chain.plain_text(), "hello");
    }

    #[test]
    fn streaming_policy_extracts_final_sendable_chain_from_delta() {
        let chain = MessageChain::new(vec![
            MessageComponent::reply("msg-1", "quoted"),
            MessageComponent::plain("final"),
        ]);
        let policy = AgentStreamingFeedbackPolicy::default().stream_to_general(true);

        let event = policy
            .streaming_delta_event(chain)
            .expect("sendable delta should become final chain");

        assert_eq!(event.kind, AgentFeedbackEventKind::FinalChain);
        assert_eq!(event.chain.plain_text(), "final");
    }

    #[test]
    fn streaming_policy_can_emit_break_before_tool_call() {
        let policy = AgentStreamingFeedbackPolicy::default();

        let event = policy
            .tool_call_break_event()
            .expect("break event should be enabled by default");

        assert_eq!(event.kind, AgentFeedbackEventKind::StreamingBreak);
        assert!(event.chain.is_empty());
    }
}
