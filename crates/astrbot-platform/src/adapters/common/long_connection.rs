use std::collections::{HashMap, hash_map::Entry};
use std::sync::Arc;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Map, Value};
use tokio::sync::{Mutex, broadcast, oneshot, watch};
use tokio::time::{self, MissedTickBehavior};
use tokio_tungstenite::{connect_async, tungstenite::Message};

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

    pub async fn cancel(&self, request_id: &str) -> bool {
        self.waiters.lock().await.remove(request_id).is_some()
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

#[derive(Clone)]
pub struct TungsteniteLongConnectionClient {
    endpoint: LongConnectionEndpoint,
    reconnect_policy: LongConnectionReconnectPolicy,
    command_timeout: Duration,
    state: Arc<Mutex<LongConnectionState>>,
    waiters: Arc<LongConnectionWaiters>,
    outbound_tx: broadcast::Sender<LongConnectionCommand>,
    frame_tx: broadcast::Sender<LongConnectionFrame>,
    shutdown_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
}

impl TungsteniteLongConnectionClient {
    pub fn new(endpoint: LongConnectionEndpoint) -> Self {
        let (outbound_tx, _) = broadcast::channel(64);
        let (frame_tx, _) = broadcast::channel(128);
        Self {
            endpoint,
            reconnect_policy: LongConnectionReconnectPolicy::default(),
            command_timeout: Duration::from_secs(30),
            state: Arc::new(Mutex::new(LongConnectionState::Disconnected)),
            waiters: Arc::new(LongConnectionWaiters::new()),
            outbound_tx,
            frame_tx,
            shutdown_tx: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_reconnect_policy(
        mut self,
        reconnect_policy: LongConnectionReconnectPolicy,
    ) -> Self {
        self.reconnect_policy = reconnect_policy;
        self
    }

    pub fn with_command_timeout(mut self, command_timeout: Duration) -> Self {
        self.command_timeout = command_timeout.max(Duration::from_millis(1));
        self
    }

    pub fn subscribe_frames(&self) -> broadcast::Receiver<LongConnectionFrame> {
        self.frame_tx.subscribe()
    }

    async fn set_state(&self, state: LongConnectionState) {
        *self.state.lock().await = state;
    }

    async fn run_connection_loop(
        &self,
        shutdown_rx: &mut watch::Receiver<bool>,
        outbound_rx: &mut broadcast::Receiver<LongConnectionCommand>,
    ) -> Result<()> {
        let (socket, _) = tokio::select! {
            changed = shutdown_rx.changed() => {
                let _ = changed;
                return Ok(());
            }
            connected = connect_async(self.endpoint.url.as_str()) => {
                connected.map_err(|err| {
                    AstrbotError::Platform(format!("connect long connection: {err}"))
                })?
            }
        };
        self.set_state(LongConnectionState::Connected {
            endpoint: self.endpoint.url.clone(),
        })
        .await;
        let (mut writer, mut reader) = socket.split();
        let mut heartbeat = time::interval(self.endpoint.heartbeat_interval);
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                changed = shutdown_rx.changed() => {
                    if changed.is_ok() && *shutdown_rx.borrow() {
                        let _ = writer.send(Message::Close(None)).await;
                        return Ok(());
                    }
                }
                outgoing = outbound_rx.recv() => {
                    match outgoing {
                        Ok(command) => {
                            writer
                                .send(command_to_message(&command))
                                .await
                                .map_err(|err| AstrbotError::Platform(format!("send long connection command: {err}")))?;
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => {
                            return Err(AstrbotError::Platform(
                                "long connection command channel closed".to_string(),
                            ));
                        }
                    }
                }
                incoming = reader.next() => {
                    let Some(incoming) = incoming else {
                        return Err(AstrbotError::Platform(
                            "long connection closed by remote".to_string(),
                        ));
                    };
                    let incoming = incoming
                        .map_err(|err| AstrbotError::Platform(format!("read long connection frame: {err}")))?;
                    match incoming {
                        Message::Text(text) => {
                            self.handle_incoming_frame(parse_text_frame(&text)).await;
                        }
                        Message::Binary(payload) => {
                            let _ = self.frame_tx.send(LongConnectionFrame::Callback {
                                command: "binary".to_string(),
                                payload: payload.to_vec(),
                            });
                        }
                        Message::Ping(payload) => {
                            writer
                                .send(Message::Pong(payload))
                                .await
                                .map_err(|err| AstrbotError::Platform(format!("send long connection pong: {err}")))?;
                            let _ = self.frame_tx.send(LongConnectionFrame::Heartbeat);
                        }
                        Message::Pong(_) => {
                            let _ = self.frame_tx.send(LongConnectionFrame::Heartbeat);
                        }
                        Message::Close(_) => {
                            return Err(AstrbotError::Platform(
                                "long connection close frame received".to_string(),
                            ));
                        }
                        Message::Frame(_) => {}
                    }
                }
                _ = heartbeat.tick() => {
                    writer
                        .send(Message::Ping(Vec::new().into()))
                        .await
                        .map_err(|err| AstrbotError::Platform(format!("send long connection heartbeat: {err}")))?;
                }
            }
        }
    }

    async fn handle_incoming_frame(&self, frame: LongConnectionFrame) {
        if !self.waiters.resolve(frame.clone()).await {
            let _ = self.frame_tx.send(frame);
        }
    }
}

#[async_trait]
impl LongConnectionClient for TungsteniteLongConnectionClient {
    async fn run(&self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);
        let mut outbound_rx = self.outbound_tx.subscribe();
        let mut attempt = 1_u32;

        loop {
            if *shutdown_rx.borrow() {
                break;
            }
            let next_state = if attempt == 1 {
                LongConnectionState::Connecting {
                    endpoint: self.endpoint.url.clone(),
                }
            } else {
                LongConnectionState::Reconnecting {
                    endpoint: self.endpoint.url.clone(),
                    attempt,
                }
            };
            self.set_state(next_state).await;

            let result = self
                .run_connection_loop(&mut shutdown_rx, &mut outbound_rx)
                .await;
            if *shutdown_rx.borrow() {
                break;
            }
            match result {
                Ok(()) => break,
                Err(error) => {
                    let Some(delay) = self.reconnect_policy.delay_for_attempt(attempt) else {
                        self.set_state(LongConnectionState::Disconnected).await;
                        *self.shutdown_tx.lock().await = None;
                        return Err(error);
                    };
                    self.set_state(LongConnectionState::Reconnecting {
                        endpoint: self.endpoint.url.clone(),
                        attempt,
                    })
                    .await;
                    attempt = attempt.saturating_add(1);
                    tokio::select! {
                        changed = shutdown_rx.changed() => {
                            let _ = changed;
                            if *shutdown_rx.borrow() {
                                break;
                            }
                        }
                        _ = time::sleep(delay) => {}
                    }
                }
            }
        }

        *self.shutdown_tx.lock().await = None;
        self.set_state(LongConnectionState::Disconnected).await;
        Ok(())
    }

    async fn terminate(&self) -> Result<()> {
        self.set_state(LongConnectionState::Closing).await;
        if let Some(sender) = self.shutdown_tx.lock().await.as_ref() {
            let _ = sender.send(true);
        }
        Ok(())
    }

    async fn send_command(&self, command: LongConnectionCommand) -> Result<LongConnectionFrame> {
        let request_id = command.request_id.clone();
        let receiver = self.waiters.register(request_id.clone()).await?;
        if self.outbound_tx.send(command).is_err() {
            self.waiters.cancel(&request_id).await;
            return Err(AstrbotError::Platform(
                "long connection client is not running".to_string(),
            ));
        }

        match time::timeout(self.command_timeout, receiver).await {
            Ok(Ok(frame)) => Ok(frame),
            Ok(Err(_)) => Err(AstrbotError::Platform(format!(
                "long connection response channel closed for request {request_id}"
            ))),
            Err(_) => {
                self.waiters.cancel(&request_id).await;
                Err(AstrbotError::Platform(format!(
                    "long connection command {request_id} timed out"
                )))
            }
        }
    }

    fn state(&self) -> LongConnectionState {
        self.state
            .try_lock()
            .map(|state| state.clone())
            .unwrap_or(LongConnectionState::Closing)
    }
}

fn command_to_message(command: &LongConnectionCommand) -> Message {
    let mut object = Map::new();
    object.insert("cmd".to_string(), Value::String(command.name.clone()));
    object.insert(
        "request_id".to_string(),
        Value::String(command.request_id.clone()),
    );
    if !command.payload.is_empty() {
        let payload = serde_json::from_slice::<Value>(&command.payload)
            .or_else(|_| {
                String::from_utf8(command.payload.clone())
                    .map(Value::String)
                    .map_err(|err| {
                        serde_json::Error::io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            err,
                        ))
                    })
            })
            .unwrap_or_else(|_| {
                Value::Array(
                    command
                        .payload
                        .iter()
                        .map(|byte| Value::from(*byte))
                        .collect(),
                )
            });
        object.insert("payload".to_string(), payload);
    }
    Message::Text(Value::Object(object).to_string().into())
}

fn parse_text_frame(text: &str) -> LongConnectionFrame {
    let payload = text.as_bytes().to_vec();
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return LongConnectionFrame::Callback {
            command: "text".to_string(),
            payload,
        };
    };
    if value_is_heartbeat(&value) {
        return LongConnectionFrame::Heartbeat;
    }
    if let Some(request_id) = value_request_id(&value) {
        return LongConnectionFrame::CommandResponse {
            request_id,
            payload,
        };
    }
    LongConnectionFrame::Callback {
        command: value_command(&value).unwrap_or_else(|| "message".to_string()),
        payload,
    }
}

fn value_is_heartbeat(value: &Value) -> bool {
    ["heartbeat", "ping", "pong"].iter().any(|expected| {
        value
            .get("cmd")
            .and_then(value_as_non_empty_string)
            .as_deref()
            == Some(*expected)
            || value
                .get("type")
                .and_then(value_as_non_empty_string)
                .as_deref()
                == Some(*expected)
    })
}

fn value_request_id(value: &Value) -> Option<String> {
    value
        .get("request_id")
        .and_then(value_as_non_empty_string)
        .or_else(|| value.get("req_id").and_then(value_as_non_empty_string))
        .or_else(|| {
            value
                .get("headers")
                .and_then(|headers| headers.get("req_id"))
                .and_then(value_as_non_empty_string)
        })
}

fn value_command(value: &Value) -> Option<String> {
    value
        .get("cmd")
        .and_then(value_as_non_empty_string)
        .or_else(|| value.get("command").and_then(value_as_non_empty_string))
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
    use std::time::Duration;

    use futures_util::{SinkExt, StreamExt};
    use serde_json::{Value, json};
    use tokio::net::TcpListener;
    use tokio::sync::broadcast;
    use tokio::time::timeout;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    use super::{
        LongConnectionClient, LongConnectionCommand, LongConnectionEndpoint, LongConnectionFrame,
        LongConnectionReconnectPolicy, LongConnectionState, LongConnectionWaiters,
        TungsteniteLongConnectionClient, command_to_message, parse_text_frame,
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

    #[test]
    fn command_message_and_text_frame_parser_keep_wire_json_outside_events() {
        let command =
            LongConnectionCommand::new("send", "req-7").with_payload(br#"{"text":"hello"}"#);
        let Message::Text(text) = command_to_message(&command) else {
            panic!("command should serialize to text frame");
        };
        let payload = serde_json::from_str::<Value>(&text).expect("command frame should be JSON");
        assert_eq!(payload["cmd"], "send");
        assert_eq!(payload["request_id"], "req-7");
        assert_eq!(payload["payload"]["text"], "hello");

        assert_eq!(
            parse_text_frame(r#"{"headers":{"req_id":"req-8"},"errcode":0}"#),
            LongConnectionFrame::CommandResponse {
                request_id: "req-8".to_string(),
                payload: br#"{"headers":{"req_id":"req-8"},"errcode":0}"#.to_vec(),
            }
        );
        assert_eq!(
            parse_text_frame(r#"{"cmd":"event_callback","data":{"ok":true}}"#),
            LongConnectionFrame::Callback {
                command: "event_callback".to_string(),
                payload: br#"{"cmd":"event_callback","data":{"ok":true}}"#.to_vec(),
            }
        );
        assert_eq!(
            parse_text_frame(r#"{"type":"heartbeat"}"#),
            LongConnectionFrame::Heartbeat
        );
    }

    #[tokio::test]
    async fn tungstenite_client_sends_command_reconnects_and_terminates() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("local addr");
        let server = tokio::spawn(run_fake_long_connection_server(listener));
        let client = TungsteniteLongConnectionClient::new(
            LongConnectionEndpoint::new(format!("ws://{address}"))
                .with_heartbeat_interval(Duration::from_secs(30)),
        )
        .with_reconnect_policy(
            LongConnectionReconnectPolicy::new(
                Duration::from_millis(10),
                Duration::from_millis(10),
            )
            .with_max_attempts(4),
        )
        .with_command_timeout(Duration::from_secs(3));
        let mut frames = client.subscribe_frames();
        let running = client.clone();
        let runner = tokio::spawn(async move { running.run().await });

        wait_for_connected(&client).await;
        wait_for_callback(&mut frames, "connected-1").await;

        let response = client
            .send_command(LongConnectionCommand::new("send", "req-1").with_payload("hello"))
            .await
            .expect("command should receive response");
        assert_eq!(
            response,
            LongConnectionFrame::CommandResponse {
                request_id: "req-1".to_string(),
                payload: br#"{"ok":true,"request_id":"req-1"}"#.to_vec(),
            }
        );

        wait_for_callback(&mut frames, "connected-2").await;
        assert!(client.state().is_connected());

        client.terminate().await.expect("terminate");
        timeout(Duration::from_secs(3), runner)
            .await
            .expect("runner should stop")
            .expect("runner should join")
            .expect("runner should succeed");
        timeout(Duration::from_secs(3), server)
            .await
            .expect("server should stop")
            .expect("server should join");
        assert_eq!(client.state(), LongConnectionState::Disconnected);
    }

    async fn run_fake_long_connection_server(listener: TcpListener) {
        for connection_index in 1..=2 {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut socket = accept_async(stream).await.expect("websocket accept");
            socket
                .send(Message::Text(
                    json!({"cmd": format!("connected-{connection_index}")})
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send callback");

            if connection_index == 1 {
                while let Some(message) = socket.next().await {
                    match message.expect("fake server message") {
                        Message::Text(text) => {
                            let payload =
                                serde_json::from_str::<Value>(&text).expect("command JSON");
                            let request_id = payload["request_id"].as_str().expect("request id");
                            socket
                                .send(Message::Text(
                                    json!({"ok": true, "request_id": request_id})
                                        .to_string()
                                        .into(),
                                ))
                                .await
                                .expect("send response");
                            let _ = socket.close(None).await;
                            break;
                        }
                        Message::Ping(payload) => {
                            socket.send(Message::Pong(payload)).await.expect("pong");
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
            } else {
                while let Some(message) = socket.next().await {
                    match message.expect("fake server message") {
                        Message::Ping(payload) => {
                            socket.send(Message::Pong(payload)).await.expect("pong");
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
                break;
            }
        }
    }

    async fn wait_for_connected(client: &TungsteniteLongConnectionClient) {
        for _ in 0..100 {
            if client.state().is_connected() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("long connection client did not connect");
    }

    async fn wait_for_callback(
        frames: &mut broadcast::Receiver<LongConnectionFrame>,
        expected: &str,
    ) {
        for _ in 0..16 {
            let frame = timeout(Duration::from_secs(3), frames.recv())
                .await
                .expect("callback should arrive")
                .expect("frame channel should stay open");
            if matches!(
                frame,
                LongConnectionFrame::Callback { ref command, .. } if command == expected
            ) {
                return;
            }
        }
        panic!("callback {expected} did not arrive");
    }
}
