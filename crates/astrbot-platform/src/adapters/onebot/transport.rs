use astrbot_core::Result;
use async_trait::async_trait;

use crate::{PlatformTransport, PlatformTransportKind, PlatformTransportState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OneBotTransportMode {
    InProcess,
    ReverseWebSocket { host: String, port: u16 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OneBotTransport {
    mode: OneBotTransportMode,
}

impl OneBotTransport {
    pub fn in_process() -> Self {
        Self {
            mode: OneBotTransportMode::InProcess,
        }
    }

    pub fn reverse_websocket(host: impl Into<String>, port: u16) -> Self {
        Self {
            mode: OneBotTransportMode::ReverseWebSocket {
                host: host.into(),
                port,
            },
        }
    }

    pub fn mode(&self) -> &OneBotTransportMode {
        &self.mode
    }
}

#[async_trait]
impl PlatformTransport for OneBotTransport {
    async fn run(&self) -> Result<()> {
        Ok(())
    }

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }

    fn state(&self) -> PlatformTransportState {
        match &self.mode {
            OneBotTransportMode::InProcess => {
                PlatformTransportState::disconnected(PlatformTransportKind::InProcess)
            }
            OneBotTransportMode::ReverseWebSocket { host, port } => {
                PlatformTransportState::disconnected(PlatformTransportKind::ReverseWebSocket)
                    .with_endpoint(format!("{host}:{port}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OneBotTransport, OneBotTransportMode};
    use crate::{PlatformTransport, PlatformTransportKind};

    #[test]
    fn reverse_websocket_transport_reports_endpoint_state() {
        let transport = OneBotTransport::reverse_websocket("127.0.0.1", 6700);

        assert_eq!(
            transport.mode(),
            &OneBotTransportMode::ReverseWebSocket {
                host: "127.0.0.1".to_string(),
                port: 6700,
            }
        );

        let state = transport.state();
        assert_eq!(state.kind, PlatformTransportKind::ReverseWebSocket);
        assert_eq!(state.endpoint.as_deref(), Some("127.0.0.1:6700"));
        assert!(!state.connected);
    }
}
