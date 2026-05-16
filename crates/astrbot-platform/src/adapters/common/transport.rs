use astrbot_core::Result;
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformTransportKind {
    InProcess,
    ReverseWebSocket,
    Webhook,
    LongConnection,
    LongPolling,
    ApiClient,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformTransportState {
    pub kind: PlatformTransportKind,
    pub connected: bool,
    pub endpoint: Option<String>,
}

impl PlatformTransportState {
    pub fn disconnected(kind: PlatformTransportKind) -> Self {
        Self {
            kind,
            connected: false,
            endpoint: None,
        }
    }

    pub fn connected(kind: PlatformTransportKind, endpoint: impl Into<String>) -> Self {
        Self {
            kind,
            connected: true,
            endpoint: Some(endpoint.into()),
        }
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }
}

#[async_trait]
pub trait PlatformTransport: Send + Sync {
    async fn run(&self) -> Result<()>;

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }

    fn state(&self) -> PlatformTransportState;
}

#[derive(Clone, Debug)]
pub struct NoopTransport {
    state: PlatformTransportState,
}

impl NoopTransport {
    pub fn new(kind: PlatformTransportKind) -> Self {
        Self {
            state: PlatformTransportState::disconnected(kind),
        }
    }
}

#[async_trait]
impl PlatformTransport for NoopTransport {
    async fn run(&self) -> Result<()> {
        Ok(())
    }

    fn state(&self) -> PlatformTransportState {
        self.state.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{NoopTransport, PlatformTransport, PlatformTransportKind};

    #[tokio::test]
    async fn noop_transport_has_disconnected_state_and_runs() {
        let transport = NoopTransport::new(PlatformTransportKind::ReverseWebSocket);

        transport.run().await.expect("noop transport should run");

        let state = transport.state();
        assert_eq!(state.kind, PlatformTransportKind::ReverseWebSocket);
        assert!(!state.connected);
        assert!(state.endpoint.is_none());
    }
}
