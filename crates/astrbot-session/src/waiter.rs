use std::collections::HashMap;
use std::time::{Duration, Instant};

use astrbot_core::MessageChain;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionWaitRegistration {
    pub session_id: String,
    pub timeout: Duration,
    pub record_history: bool,
    expires_at: Instant,
    history: Vec<MessageChain>,
}

impl SessionWaitRegistration {
    pub fn new(session_id: impl Into<String>, timeout: Duration, record_history: bool) -> Self {
        Self {
            session_id: session_id.into(),
            timeout,
            record_history,
            expires_at: Instant::now() + timeout,
            history: Vec::new(),
        }
    }

    pub fn history(&self) -> &[MessageChain] {
        &self.history
    }

    pub fn keep(&mut self, timeout: Duration, reset_timeout: bool) -> SessionWaitDecision {
        let now = Instant::now();
        let next_timeout = if reset_timeout {
            timeout
        } else {
            let remaining = self.expires_at.saturating_duration_since(now);
            remaining + timeout
        };

        if next_timeout.is_zero() {
            self.expires_at = now;
            SessionWaitDecision::TimedOut
        } else {
            self.timeout = next_timeout;
            self.expires_at = now + next_timeout;
            SessionWaitDecision::Waiting
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    fn push_history(&mut self, chain: MessageChain) {
        if self.record_history {
            self.history.push(chain);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWaitingEvent {
    pub session_id: String,
    pub chain: MessageChain,
}

impl SessionWaitingEvent {
    pub fn new(session_id: impl Into<String>, chain: MessageChain) -> Self {
        Self {
            session_id: session_id.into(),
            chain,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionWaitDecision {
    Waiting,
    Triggered,
    Missing,
    TimedOut,
}

#[derive(Default)]
pub struct SessionWaiter {
    waits: HashMap<String, SessionWaitRegistration>,
}

impl SessionWaiter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        session_id: impl Into<String>,
        timeout: Duration,
        record_history: bool,
    ) -> SessionWaitDecision {
        let registration = SessionWaitRegistration::new(session_id, timeout, record_history);
        self.waits
            .insert(registration.session_id.clone(), registration);
        SessionWaitDecision::Waiting
    }

    pub fn trigger(&mut self, event: SessionWaitingEvent) -> SessionWaitDecision {
        let Some(wait) = self.waits.get_mut(&event.session_id) else {
            return SessionWaitDecision::Missing;
        };

        if wait.is_expired() {
            self.waits.remove(&event.session_id);
            return SessionWaitDecision::TimedOut;
        }

        wait.push_history(event.chain);
        SessionWaitDecision::Triggered
    }

    pub fn keep(
        &mut self,
        session_id: &str,
        timeout: Duration,
        reset_timeout: bool,
    ) -> SessionWaitDecision {
        let Some(wait) = self.waits.get_mut(session_id) else {
            return SessionWaitDecision::Missing;
        };
        let decision = wait.keep(timeout, reset_timeout);
        if decision == SessionWaitDecision::TimedOut {
            self.waits.remove(session_id);
        }
        decision
    }

    pub fn finish(&mut self, session_id: &str) -> Option<SessionWaitRegistration> {
        self.waits.remove(session_id)
    }

    pub fn history(&self, session_id: &str) -> Option<&[MessageChain]> {
        self.waits
            .get(session_id)
            .map(SessionWaitRegistration::history)
    }

    pub fn has_wait(&self, session_id: &str) -> bool {
        self.waits.contains_key(session_id)
    }
}
