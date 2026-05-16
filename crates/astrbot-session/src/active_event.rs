use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveEventRecord {
    pub event_id: String,
    pub session_id: String,
    pub stop_event_requested: bool,
    pub agent_stop_requested: bool,
}

impl ActiveEventRecord {
    pub fn new(event_id: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            event_id: event_id.into(),
            session_id: session_id.into(),
            stop_event_requested: false,
            agent_stop_requested: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveEventHandle {
    event_id: String,
    session_id: String,
}

impl ActiveEventHandle {
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveEventInterruption {
    StopEvent,
    RequestAgentStop,
}

#[derive(Default)]
pub struct ActiveEventRegistry {
    events: HashMap<String, ActiveEventRecord>,
    by_session: HashMap<String, HashSet<String>>,
}

impl ActiveEventRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        event_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> ActiveEventHandle {
        let record = ActiveEventRecord::new(event_id, session_id);
        let handle = ActiveEventHandle {
            event_id: record.event_id.clone(),
            session_id: record.session_id.clone(),
        };
        self.by_session
            .entry(record.session_id.clone())
            .or_default()
            .insert(record.event_id.clone());
        self.events.insert(record.event_id.clone(), record);
        handle
    }

    pub fn unregister(&mut self, event_id: &str) -> Option<ActiveEventRecord> {
        let record = self.events.remove(event_id)?;
        if let Some(session_events) = self.by_session.get_mut(&record.session_id) {
            session_events.remove(event_id);
            if session_events.is_empty() {
                self.by_session.remove(&record.session_id);
            }
        }
        Some(record)
    }

    pub fn interrupt_session(
        &mut self,
        session_id: &str,
        interruption: ActiveEventInterruption,
        exclude_event_id: Option<&str>,
    ) -> usize {
        let Some(event_ids) = self.by_session.get(session_id) else {
            return 0;
        };
        let event_ids = event_ids.iter().cloned().collect::<Vec<_>>();
        let mut count = 0;

        for event_id in event_ids {
            if exclude_event_id == Some(event_id.as_str()) {
                continue;
            }
            let Some(record) = self.events.get_mut(&event_id) else {
                continue;
            };
            match interruption {
                ActiveEventInterruption::StopEvent => record.stop_event_requested = true,
                ActiveEventInterruption::RequestAgentStop => record.agent_stop_requested = true,
            }
            count += 1;
        }

        count
    }

    pub fn record(&self, event_id: &str) -> Option<&ActiveEventRecord> {
        self.events.get(event_id)
    }

    pub fn session_event_count(&self, session_id: &str) -> usize {
        self.by_session.get(session_id).map_or(0, HashSet::len)
    }
}
