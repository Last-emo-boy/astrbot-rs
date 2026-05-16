use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::log_buffer::{LogEntry, LogLevel, LogSource};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceEvent {
    pub span_id: String,
    pub span_name: String,
    pub action: String,
    pub message_origin: Option<String>,
    pub sender_name: Option<String>,
    pub message_outline: Option<String>,
    pub fields: Vec<(String, String)>,
    pub occurred_at: SystemTime,
    pub elapsed: Option<Duration>,
}

impl TraceEvent {
    pub fn to_log_entry(&self) -> LogEntry {
        LogEntry::new(
            LogLevel::Trace,
            LogSource::Trace,
            format!("{}: {}", self.span_name, self.action),
        )
        .with_target(self.span_id.clone())
    }
}

pub trait TraceSink: Send + Sync {
    fn record(&self, event: TraceEvent);
}

pub type SharedTraceSink = Arc<dyn TraceSink>;

#[derive(Debug, Default)]
pub struct NoopTraceSink;

impl TraceSink for NoopTraceSink {
    fn record(&self, _event: TraceEvent) {}
}

#[derive(Debug, Default)]
pub struct InMemoryTraceSink {
    events: Mutex<Vec<TraceEvent>>,
}

impl InMemoryTraceSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<TraceEvent> {
        self.events.lock().expect("trace sink poisoned").clone()
    }
}

impl TraceSink for InMemoryTraceSink {
    fn record(&self, event: TraceEvent) {
        self.events.lock().expect("trace sink poisoned").push(event);
    }
}

#[derive(Clone)]
pub struct TraceSpan {
    span_id: String,
    span_name: String,
    message_origin: Option<String>,
    sender_name: Option<String>,
    message_outline: Option<String>,
    started_at: SystemTime,
    sink: SharedTraceSink,
}

impl TraceSpan {
    pub fn new(
        span_id: impl Into<String>,
        span_name: impl Into<String>,
        sink: SharedTraceSink,
    ) -> Self {
        Self {
            span_id: span_id.into(),
            span_name: span_name.into(),
            message_origin: None,
            sender_name: None,
            message_outline: None,
            started_at: SystemTime::now(),
            sink,
        }
    }

    pub fn with_message_origin(mut self, message_origin: impl Into<String>) -> Self {
        self.message_origin = Some(message_origin.into());
        self
    }

    pub fn with_sender_name(mut self, sender_name: impl Into<String>) -> Self {
        self.sender_name = Some(sender_name.into());
        self
    }

    pub fn with_message_outline(mut self, message_outline: impl Into<String>) -> Self {
        self.message_outline = Some(message_outline.into());
        self
    }

    pub fn record(&self, action: impl Into<String>, fields: Vec<(String, String)>) {
        let now = SystemTime::now();
        self.sink.record(TraceEvent {
            span_id: self.span_id.clone(),
            span_name: self.span_name.clone(),
            action: action.into(),
            message_origin: self.message_origin.clone(),
            sender_name: self.sender_name.clone(),
            message_outline: self.message_outline.clone(),
            fields,
            occurred_at: now,
            elapsed: now.duration_since(self.started_at).ok(),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{InMemoryTraceSink, TraceSpan};

    #[test]
    fn trace_span_records_actions_with_context() {
        let sink = Arc::new(InMemoryTraceSink::new());
        let span = TraceSpan::new("span-1", "pipeline", sink.clone())
            .with_message_origin("webchat:user")
            .with_sender_name("alice")
            .with_message_outline("hello");

        span.record(
            "wake-check",
            vec![("matched".to_string(), "true".to_string())],
        );

        let events = sink.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].span_id, "span-1");
        assert_eq!(events[0].sender_name.as_deref(), Some("alice"));
        assert_eq!(
            events[0].fields,
            vec![("matched".to_string(), "true".to_string())]
        );
    }
}
