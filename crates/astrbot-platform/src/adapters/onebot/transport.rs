use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use astrbot_core::{AstrbotError, MessageEvent, MessageSink, Result};
use async_trait::async_trait;
use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};

use crate::{PlatformTransport, PlatformTransportKind, PlatformTransportState};

use super::event::build_onebot_event;
use super::message::parse_onebot_message_chain;
use super::session::OneBotSession;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OneBotTransportMode {
    InProcess,
    ReverseWebSocket {
        host: String,
        port: u16,
        token: Option<String>,
    },
}

pub struct OneBotTransport {
    inner: Arc<OneBotTransportInner>,
}

struct OneBotTransportInner {
    mode: OneBotTransportMode,
    connected: Arc<AtomicBool>,
    outbound_tx: broadcast::Sender<Value>,
    shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
}

impl Clone for OneBotTransport {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl fmt::Debug for OneBotTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OneBotTransport")
            .field("mode", self.mode())
            .finish()
    }
}

impl PartialEq for OneBotTransport {
    fn eq(&self, other: &Self) -> bool {
        self.mode() == other.mode()
    }
}

impl Eq for OneBotTransport {}

pub(super) struct OneBotReverseWebSocketContext {
    pub platform_id: String,
    pub platform_name: String,
    pub event_sender: mpsc::Sender<MessageEvent>,
    pub sink: Arc<dyn MessageSink>,
    pub event_counter: Arc<AtomicU64>,
}

impl OneBotTransport {
    pub fn in_process() -> Self {
        Self::new(OneBotTransportMode::InProcess)
    }

    pub fn reverse_websocket(host: impl Into<String>, port: u16) -> Self {
        Self::reverse_websocket_with_token(host, port, None::<String>)
    }

    pub fn reverse_websocket_with_token(
        host: impl Into<String>,
        port: u16,
        token: impl Into<Option<String>>,
    ) -> Self {
        let token = token
            .into()
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty());
        Self::new(OneBotTransportMode::ReverseWebSocket {
            host: host.into(),
            port,
            token,
        })
    }

    fn new(mode: OneBotTransportMode) -> Self {
        let (outbound_tx, _) = broadcast::channel(64);
        Self {
            inner: Arc::new(OneBotTransportInner {
                mode,
                connected: Arc::new(AtomicBool::new(false)),
                outbound_tx,
                shutdown_tx: Mutex::new(None),
            }),
        }
    }

    pub fn mode(&self) -> &OneBotTransportMode {
        &self.inner.mode
    }

    pub fn is_reverse_websocket(&self) -> bool {
        matches!(self.mode(), OneBotTransportMode::ReverseWebSocket { .. })
    }

    pub async fn send_action(&self, action: Value) -> Result<()> {
        if !self.is_reverse_websocket() {
            return Ok(());
        }
        if !self.inner.connected.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.inner
            .outbound_tx
            .send(action)
            .map(|_| ())
            .map_err(|_| {
                AstrbotError::Platform("onebot outbound websocket send failed".to_string())
            })
    }

    pub(super) async fn run_with_context(&self, ctx: OneBotReverseWebSocketContext) -> Result<()> {
        match self.mode() {
            OneBotTransportMode::InProcess => Ok(()),
            OneBotTransportMode::ReverseWebSocket { host, port, token } => {
                self.run_reverse_websocket(host.clone(), *port, token.clone(), ctx)
                    .await
            }
        }
    }

    async fn run_reverse_websocket(
        &self,
        host: String,
        port: u16,
        token: Option<String>,
        ctx: OneBotReverseWebSocketContext,
    ) -> Result<()> {
        let listener = TcpListener::bind((host.as_str(), port))
            .await
            .map_err(|err| {
                AstrbotError::Platform(format!("bind onebot reverse websocket: {err}"))
            })?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        *self.inner.shutdown_tx.lock().await = Some(shutdown_tx);
        let state = Arc::new(OneBotWebSocketState {
            token,
            platform_id: ctx.platform_id,
            platform_name: ctx.platform_name,
            event_sender: ctx.event_sender,
            sink: ctx.sink,
            event_counter: ctx.event_counter,
            connected: self.inner.connected.clone(),
            outbound_tx: self.inner.outbound_tx.clone(),
        });
        let app = Router::new()
            .route("/", get(onebot_ws_handler))
            .route("/ws", get(onebot_ws_handler))
            .with_state(state);

        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .map_err(|err| {
                AstrbotError::Platform(format!("run onebot reverse websocket: {err}"))
            })?;
        self.inner.connected.store(false, Ordering::SeqCst);
        Ok(())
    }
}

#[async_trait]
impl PlatformTransport for OneBotTransport {
    async fn run(&self) -> Result<()> {
        match self.mode() {
            OneBotTransportMode::InProcess => Ok(()),
            OneBotTransportMode::ReverseWebSocket { .. } => Err(AstrbotError::Platform(
                "onebot reverse websocket transport requires platform context".to_string(),
            )),
        }
    }

    async fn terminate(&self) -> Result<()> {
        if let Some(shutdown_tx) = self.inner.shutdown_tx.lock().await.take() {
            let _ = shutdown_tx.send(());
        }
        self.inner.connected.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn state(&self) -> PlatformTransportState {
        match self.mode() {
            OneBotTransportMode::InProcess => {
                PlatformTransportState::disconnected(PlatformTransportKind::InProcess)
            }
            OneBotTransportMode::ReverseWebSocket { host, port, .. } => {
                let endpoint = format!("{host}:{port}");
                if self.inner.connected.load(Ordering::SeqCst) {
                    PlatformTransportState::connected(
                        PlatformTransportKind::ReverseWebSocket,
                        endpoint,
                    )
                } else {
                    PlatformTransportState::disconnected(PlatformTransportKind::ReverseWebSocket)
                        .with_endpoint(endpoint)
                }
            }
        }
    }
}

struct OneBotWebSocketState {
    token: Option<String>,
    platform_id: String,
    platform_name: String,
    event_sender: mpsc::Sender<MessageEvent>,
    sink: Arc<dyn MessageSink>,
    event_counter: Arc<AtomicU64>,
    connected: Arc<AtomicBool>,
    outbound_tx: broadcast::Sender<Value>,
}

impl OneBotWebSocketState {
    fn authorized(&self, query: &HashMap<String, String>, headers: &HeaderMap) -> bool {
        let Some(token) = self.token.as_deref() else {
            return true;
        };
        query
            .get("access_token")
            .is_some_and(|candidate| candidate == token)
            || headers
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
                .is_some_and(|candidate| candidate == token)
    }
}

async fn onebot_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<OneBotWebSocketState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !state.authorized(&query, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    ws.on_upgrade(move |socket| handle_onebot_socket(socket, state))
        .into_response()
}

async fn handle_onebot_socket(mut socket: WebSocket, state: Arc<OneBotWebSocketState>) {
    state.connected.store(true, Ordering::SeqCst);
    let mut outbound_rx = state.outbound_tx.subscribe();

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if process_incoming_text(&text, &state).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            outbound = outbound_rx.recv() => {
                match outbound {
                    Ok(action) => {
                        if socket.send(Message::Text(action.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    state.connected.store(false, Ordering::SeqCst);
}

async fn process_incoming_text(text: &str, state: &OneBotWebSocketState) -> Result<()> {
    let payload = serde_json::from_str::<Value>(text)
        .map_err(|err| AstrbotError::Platform(format!("parse onebot websocket event: {err}")))?;
    if payload.get("post_type").and_then(Value::as_str) != Some("message") {
        return Ok(());
    }

    let message_type = payload
        .get("message_type")
        .and_then(Value::as_str)
        .unwrap_or("private");
    let sender_id = payload
        .get("user_id")
        .and_then(value_as_non_empty_string)
        .unwrap_or_else(|| "unknown".to_string());
    let session = match message_type {
        "group" => {
            let group_id = payload
                .get("group_id")
                .and_then(value_as_non_empty_string)
                .ok_or_else(|| {
                    AstrbotError::Platform("onebot group message requires group_id".to_string())
                })?;
            OneBotSession::group(&state.platform_id, group_id)
        }
        _ => OneBotSession::private(&state.platform_id, sender_id.clone()),
    };
    let message_value = payload
        .get("message")
        .or_else(|| payload.get("raw_message"))
        .unwrap_or(&Value::Null);
    let chain = parse_onebot_message_chain(message_value);
    if chain.is_empty() {
        return Ok(());
    }

    let message_id = payload
        .get("message_id")
        .and_then(value_as_non_empty_string)
        .unwrap_or_else(|| {
            state
                .event_counter
                .fetch_add(1, Ordering::Relaxed)
                .to_string()
        });
    let mut event = build_onebot_event(
        format!("{}-event-{message_id}", state.platform_id),
        state.platform_id.clone(),
        state.platform_name.clone(),
        session.message_session(),
        sender_id,
        chain,
        state.sink.clone(),
    );
    if let Some(self_id) = payload.get("self_id").and_then(value_as_non_empty_string) {
        event = event.with_self_id(self_id);
    }

    state
        .event_sender
        .send(event)
        .await
        .map_err(|_| AstrbotError::EventChannelClosed)
}

fn value_as_non_empty_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

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
                token: None,
            }
        );

        let state = transport.state();
        assert_eq!(state.kind, PlatformTransportKind::ReverseWebSocket);
        assert_eq!(state.endpoint.as_deref(), Some("127.0.0.1:6700"));
        assert!(!state.connected);
    }

    #[tokio::test]
    async fn reverse_websocket_send_action_without_connection_is_noop() {
        let transport = OneBotTransport::reverse_websocket("127.0.0.1", 6700);

        transport
            .send_action(json!({"action": "send_private_msg"}))
            .await
            .expect("disconnected reverse websocket send should not fail pipeline processing");
    }
}
