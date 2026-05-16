use std::collections::{HashMap, hash_map::Entry};
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use tokio::sync::{Mutex, oneshot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LongConnectionEndpoint {
    pub url: String,
    pub heartbeat_interval: Duration,
}

impl LongConnectionEndpoint {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            heartbeat_interval: Duration::from_secs(30),
        }
    }

    pub fn with_heartbeat_interval(mut self, heartbeat_interval: Duration) -> Self {
        self.heartbeat_interval = heartbeat_interval.max(Duration::from_secs(1));
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LongConnectionState {
    Disconnected,
    Connecting { endpoint: String },
    Connected { endpoint: String },
    Reconnecting { endpoint: String, attempt: u32 },
    Closing,
}

impl LongConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    pub fn endpoint(&self) -> Option<&str> {
        match self {
            Self::Connecting { endpoint }
            | Self::Connected { endpoint }
            | Self::Reconnecting { endpoint, .. } => Some(endpoint.as_str()),
            Self::Disconnected | Self::Closing => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LongConnectionReconnectPolicy {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub max_attempts: Option<u32>,
}

impl LongConnectionReconnectPolicy {
    pub fn new(initial_delay: Duration, max_delay: Duration) -> Self {
        let initial_delay = initial_delay.max(Duration::from_millis(1));
        Self {
            initial_delay,
            max_delay: max_delay.max(initial_delay),
            max_attempts: None,
        }
    }

    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = Some(max_attempts);
        self
    }

    pub fn delay_for_attempt(&self, attempt: u32) -> Option<Duration> {
        if let Some(max_attempts) = self.max_attempts
            && attempt > max_attempts
        {
            return None;
        }

        let shift = attempt.saturating_sub(1).min(63);
        let multiplier = 1_u32.checked_shl(shift).unwrap_or(u32::MAX);
        Some(
            self.initial_delay
                .saturating_mul(multiplier)
                .min(self.max_delay),
        )
    }
}

impl Default for LongConnectionReconnectPolicy {
    fn default() -> Self {
        Self::new(Duration::from_secs(1), Duration::from_secs(30))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LongConnectionCommand {
    pub name: String,
    pub request_id: String,
    pub payload: Vec<u8>,
}

impl LongConnectionCommand {
    pub fn new(name: impl Into<String>, request_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            request_id: request_id.into(),
            payload: Vec::new(),
        }
    }

    pub fn with_payload(mut self, payload: impl Into<Vec<u8>>) -> Self {
        self.payload = payload.into();
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LongConnectionFrame {
    Callback {
        command: String,
        payload: Vec<u8>,
    },
    CommandResponse {
        request_id: String,
        payload: Vec<u8>,
    },
    Heartbeat,
}

impl LongConnectionFrame {
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::CommandResponse { request_id, .. } => Some(request_id.as_str()),
            Self::Callback { .. } | Self::Heartbeat => None,
        }
    }
}

#[derive(Default)]
pub struct LongConnectionWaiters {
    waiters: Mutex<HashMap<String, oneshot::Sender<LongConnectionFrame>>>,
}

impl LongConnectionWaiters {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(
        &self,
        request_id: impl Into<String>,
    ) -> Result<oneshot::Receiver<LongConnectionFrame>> {
        let request_id = request_id.into();
        let (sender, receiver) = oneshot::channel();
        let mut waiters = self.waiters.lock().await;
        match waiters.entry(request_id) {
            Entry::Occupied(entry) => Err(AstrbotError::Platform(format!(
                "long connection request {} is already waiting",
                entry.key()
            ))),
            Entry::Vacant(entry) => {
                entry.insert(sender);
                Ok(receiver)
            }
        }
    }

    pub async fn resolve(&self, frame: LongConnectionFrame) -> bool {
        let Some(request_id) = frame.request_id().map(str::to_string) else {
            return false;
        };
        let waiter = self.waiters.lock().await.remove(&request_id);
        waiter.is_some_and(|sender| sender.send(frame).is_ok())
    }

    pub async fn len(&self) -> usize {
        self.waiters.lock().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

#[async_trait]
pub trait LongConnectionClient: Send + Sync {
    async fn run(&self) -> Result<()>;

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }

    async fn send_command(&self, command: LongConnectionCommand) -> Result<LongConnectionFrame>;

    fn state(&self) -> LongConnectionState;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        LongConnectionCommand, LongConnectionEndpoint, LongConnectionFrame,
        LongConnectionReconnectPolicy, LongConnectionState, LongConnectionWaiters,
    };

    #[test]
    fn endpoint_and_state_keep_lifecycle_outside_manager() {
        let endpoint = LongConnectionEndpoint::new("wss://example.test/long")
            .with_heartbeat_interval(Duration::from_secs(5));
        let state = LongConnectionState::Connected {
            endpoint: endpoint.url.clone(),
        };

        assert_eq!(endpoint.heartbeat_interval, Duration::from_secs(5));
        assert!(state.is_connected());
        assert_eq!(state.endpoint(), Some("wss://example.test/long"));
    }

    #[test]
    fn reconnect_policy_caps_exponential_backoff() {
        let policy = LongConnectionReconnectPolicy::new(
            Duration::from_millis(100),
            Duration::from_millis(500),
        )
        .with_max_attempts(4);

        assert_eq!(
            policy.delay_for_attempt(1),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            policy.delay_for_attempt(2),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            policy.delay_for_attempt(4),
            Some(Duration::from_millis(500))
        );
        assert_eq!(policy.delay_for_attempt(5), None);
    }

    #[test]
    fn command_and_frame_keep_wire_payload_separate() {
        let command =
            LongConnectionCommand::new("ping", "req-1").with_payload(br#"{"ok":true}"#.to_vec());
        let frame = LongConnectionFrame::CommandResponse {
            request_id: "req-1".to_string(),
            payload: br#"{"errcode":0}"#.to_vec(),
        };

        assert_eq!(command.name, "ping");
        assert_eq!(command.request_id, "req-1");
        assert_eq!(frame.request_id(), Some("req-1"));
    }

    #[tokio::test]
    async fn waiters_resolve_matching_command_response() {
        let waiters = LongConnectionWaiters::new();
        let receiver = waiters
            .register("req-1")
            .await
            .expect("waiter should register");
        let frame = LongConnectionFrame::CommandResponse {
            request_id: "req-1".to_string(),
            payload: b"ok".to_vec(),
        };

        assert!(waiters.resolve(frame.clone()).await);
        assert_eq!(receiver.await.expect("response should be delivered"), frame);
        assert!(waiters.is_empty().await);
    }
}
