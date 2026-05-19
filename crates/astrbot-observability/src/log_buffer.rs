use std::collections::VecDeque;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct LogEntryId(pub u64);

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogSource {
    Runtime,
    Platform,
    Provider,
    Plugin,
    Dashboard,
    Trace,
    Custom(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogEntry {
    pub id: LogEntryId,
    pub level: LogLevel,
    pub source: LogSource,
    pub target: Option<String>,
    pub message: String,
    pub occurred_at: SystemTime,
}

impl LogEntry {
    pub fn new(level: LogLevel, source: LogSource, message: impl Into<String>) -> Self {
        Self {
            id: LogEntryId(0),
            level,
            source,
            target: None,
            message: message.into(),
            occurred_at: SystemTime::now(),
        }
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogBufferSnapshot {
    pub entries: Vec<LogEntry>,
    pub next_cursor: Option<LogEntryId>,
}

#[derive(Debug)]
pub struct InMemoryLogBuffer {
    capacity: usize,
    state: Mutex<LogBufferState>,
}

#[derive(Debug)]
struct LogBufferState {
    next_id: u64,
    entries: VecDeque<LogEntry>,
}

impl InMemoryLogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            state: Mutex::new(LogBufferState {
                next_id: 1,
                entries: VecDeque::new(),
            }),
        }
    }

    pub async fn push(&self, mut entry: LogEntry) -> LogEntryId {
        let mut state = self.state.lock().await;
        let id = LogEntryId(state.next_id);
        state.next_id += 1;
        entry.id = id;
        state.entries.push_back(entry);

        while state.entries.len() > self.capacity {
            state.entries.pop_front();
        }

        id
    }

    pub async fn restore(&self, entries: impl IntoIterator<Item = LogEntry>) {
        let mut state = self.state.lock().await;
        for entry in entries {
            state.next_id = state.next_id.max(entry.id.0.saturating_add(1));
            state.entries.push_back(entry);
            while state.entries.len() > self.capacity {
                state.entries.pop_front();
            }
        }
    }

    pub async fn snapshot(&self, after: Option<LogEntryId>, limit: usize) -> LogBufferSnapshot {
        let state = self.state.lock().await;
        let entries = state
            .entries
            .iter()
            .filter(|entry| after.is_none_or(|cursor| entry.id > cursor))
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_cursor = entries.last().map(|entry| entry.id);

        LogBufferSnapshot {
            entries,
            next_cursor,
        }
    }

    pub async fn len(&self) -> usize {
        self.state.lock().await.entries.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.state.lock().await.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::{InMemoryLogBuffer, LogEntry, LogEntryId, LogLevel, LogSource};

    #[tokio::test]
    async fn buffer_is_bounded_and_cursor_readable() {
        let buffer = InMemoryLogBuffer::new(2);

        buffer
            .push(LogEntry::new(LogLevel::Info, LogSource::Runtime, "one"))
            .await;
        let second = buffer
            .push(LogEntry::new(LogLevel::Warn, LogSource::Provider, "two"))
            .await;
        let third = buffer
            .push(LogEntry::new(LogLevel::Error, LogSource::Platform, "three"))
            .await;

        assert_eq!(buffer.len().await, 2);

        let all = buffer.snapshot(None, 10).await;
        assert_eq!(all.entries.len(), 2);
        assert_eq!(all.entries[0].id, second);
        assert_eq!(all.next_cursor, Some(third));

        let after_second = buffer.snapshot(Some(LogEntryId(second.0)), 10).await;
        assert_eq!(after_second.entries.len(), 1);
        assert_eq!(after_second.entries[0].message, "three");
    }

    #[tokio::test]
    async fn restore_preserves_cursor_and_capacity() {
        let buffer = InMemoryLogBuffer::new(2);
        buffer
            .restore([
                LogEntry::new(LogLevel::Info, LogSource::Runtime, "one"),
                LogEntry {
                    id: LogEntryId(7),
                    ..LogEntry::new(LogLevel::Warn, LogSource::Runtime, "seven")
                },
            ])
            .await;

        let id = buffer
            .push(LogEntry::new(LogLevel::Error, LogSource::Runtime, "next"))
            .await;
        assert_eq!(id, LogEntryId(8));

        let snapshot = buffer.snapshot(None, 10).await;
        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(snapshot.entries[0].message, "seven");
        assert_eq!(snapshot.entries[1].message, "next");
    }
}
