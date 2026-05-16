use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComponentKind {
    Runtime,
    EventBus,
    Platform,
    Provider,
    Plugin,
    Task,
    Dashboard,
    Storage,
    Custom(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComponentStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Reloading,
    Restarting,
    Failed,
    Skipped,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StatusSeverity {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusEvent {
    pub component: ComponentKind,
    pub component_id: Option<String>,
    pub status: ComponentStatus,
    pub severity: StatusSeverity,
    pub message: Option<String>,
    pub occurred_at: SystemTime,
}

impl StatusEvent {
    pub fn new(component: ComponentKind, status: ComponentStatus) -> Self {
        Self {
            component,
            component_id: None,
            status,
            severity: StatusSeverity::Info,
            message: None,
            occurred_at: SystemTime::now(),
        }
    }

    pub fn with_component_id(mut self, component_id: impl Into<String>) -> Self {
        self.component_id = Some(component_id.into());
        self
    }

    pub fn with_severity(mut self, severity: StatusSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

pub trait StatusEventSink: Send + Sync {
    fn emit(&self, event: StatusEvent);
}

pub type SharedStatusEventSink = Arc<dyn StatusEventSink>;

#[derive(Debug, Default)]
pub struct NoopStatusEventSink;

impl StatusEventSink for NoopStatusEventSink {
    fn emit(&self, _event: StatusEvent) {}
}

#[derive(Debug, Default)]
pub struct InMemoryStatusCollector {
    events: Mutex<Vec<StatusEvent>>,
}

impl InMemoryStatusCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<StatusEvent> {
        self.events
            .lock()
            .expect("status collector poisoned")
            .clone()
    }

    pub fn clear(&self) {
        self.events
            .lock()
            .expect("status collector poisoned")
            .clear();
    }
}

impl StatusEventSink for InMemoryStatusCollector {
    fn emit(&self, event: StatusEvent) {
        self.events
            .lock()
            .expect("status collector poisoned")
            .push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ComponentKind, ComponentStatus, InMemoryStatusCollector, StatusEvent, StatusEventSink,
        StatusSeverity,
    };

    #[test]
    fn collector_records_typed_status_events() {
        let collector = InMemoryStatusCollector::new();

        collector.emit(
            StatusEvent::new(ComponentKind::Platform, ComponentStatus::Starting)
                .with_component_id("webchat")
                .with_severity(StatusSeverity::Debug)
                .with_message("starting platform"),
        );

        let events = collector.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].component, ComponentKind::Platform);
        assert_eq!(events[0].component_id.as_deref(), Some("webchat"));
        assert_eq!(events[0].status, ComponentStatus::Starting);
        assert_eq!(events[0].severity, StatusSeverity::Debug);
    }
}
