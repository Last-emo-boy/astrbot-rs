use crate::MessageEvent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventLogRecord {
    pub event_id: String,
    pub platform_id: String,
    pub platform_name: String,
    pub conversation_id: String,
    pub sender_id: String,
    pub sender_name: Option<String>,
    pub message_outline: String,
}

impl EventLogRecord {
    pub fn from_event(event: &MessageEvent) -> Self {
        Self {
            event_id: event.id.clone(),
            platform_id: event.platform_id.clone(),
            platform_name: event.platform_name.clone(),
            conversation_id: event.session.conversation_id.clone(),
            sender_id: event.sender.id.clone(),
            sender_name: event.sender.display_name.clone(),
            message_outline: event.message_outline(),
        }
    }

    pub fn display_line(&self) -> String {
        match self.sender_name.as_deref().filter(|name| !name.is_empty()) {
            Some(name) => format!(
                "[{}({})] {}/{}: {}",
                self.platform_id, self.platform_name, name, self.sender_id, self.message_outline
            ),
            None => format!(
                "[{}({})] {}: {}",
                self.platform_id, self.platform_name, self.sender_id, self.message_outline
            ),
        }
    }
}

pub trait EventLogger: Send + Sync {
    fn log_event(&self, record: &EventLogRecord);
}

pub struct NoopEventLogger;

impl EventLogger for NoopEventLogger {
    fn log_event(&self, _record: &EventLogRecord) {}
}
