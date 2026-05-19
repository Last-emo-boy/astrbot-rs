use std::collections::{HashMap, hash_map::Entry};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Method, Request, StatusCode, header::CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, oneshot};

use super::security::{WebhookSignatureInput, WebhookSignatureVerdict, WebhookSignatureVerifier};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebhookHttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl WebhookHttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookEndpoint {
    pub path: String,
    pub methods: Vec<WebhookHttpMethod>,
}

impl WebhookEndpoint {
    pub fn new(path: impl Into<String>, method: WebhookHttpMethod) -> Self {
        Self {
            path: path.into(),
            methods: vec![method],
        }
    }

    pub fn get(path: impl Into<String>) -> Self {
        Self::new(path, WebhookHttpMethod::Get)
    }

    pub fn post(path: impl Into<String>) -> Self {
        Self::new(path, WebhookHttpMethod::Post)
    }

    pub fn with_method(mut self, method: WebhookHttpMethod) -> Self {
        if !self.methods.contains(&method) {
            self.methods.push(method);
        }
        self
    }

    pub fn supports(&self, method: &WebhookHttpMethod) -> bool {
        self.methods.contains(method)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookRequest {
    pub method: WebhookHttpMethod,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl WebhookRequest {
    pub fn new(method: WebhookHttpMethod, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            query: Vec::new(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn with_query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.push((key.into(), value.into()));
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }

    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    pub fn query_value(&self, key: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value.as_str())
    }

    pub fn header_value(&self, key: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
}

impl WebhookResponse {
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            body: body.into(),
            content_type: None,
        }
    }

    pub fn text(status: u16, body: impl Into<String>) -> Self {
        Self::new(status, body.into().into_bytes()).with_content_type("text/plain")
    }

    pub fn ok_text(body: impl Into<String>) -> Self {
        Self::text(200, body)
    }

    pub fn accepted() -> Self {
        Self::text(202, "accepted")
    }

    pub fn bad_request(body: impl Into<String>) -> Self {
        Self::text(400, body)
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn body_text(&self) -> Result<&str> {
        std::str::from_utf8(&self.body)
            .map_err(|err| AstrbotError::Platform(format!("webhook response is not UTF-8: {err}")))
    }
}

#[async_trait]
pub trait WebhookCallbackHandler: Send + Sync {
    async fn handle(&self, request: WebhookRequest) -> Result<WebhookResponse>;
}

#[derive(Clone)]
pub struct WebhookRoute {
    pub endpoint: WebhookEndpoint,
    pub handler: Arc<dyn WebhookCallbackHandler>,
    pub signature_verifier: Option<Arc<dyn WebhookSignatureVerifier>>,
}

impl WebhookRoute {
    pub fn new(endpoint: WebhookEndpoint, handler: Arc<dyn WebhookCallbackHandler>) -> Self {
        Self {
            endpoint,
            handler,
            signature_verifier: None,
        }
    }

    pub fn with_signature_verifier(mut self, verifier: Arc<dyn WebhookSignatureVerifier>) -> Self {
        self.signature_verifier = Some(verifier);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebhookServerState {
    Stopped,
    Starting,
    Running { endpoint_count: usize },
    Stopping,
}

#[async_trait]
pub trait WebhookServer: Send + Sync {
    async fn run(&self, routes: Vec<WebhookRoute>) -> Result<()>;

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }

    fn state(&self) -> WebhookServerState;
}

#[derive(Debug)]
pub struct AxumWebhookServer {
    bind_addr: SocketAddr,
    state: Arc<Mutex<WebhookServerState>>,
    shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    local_addr: Arc<Mutex<Option<SocketAddr>>>,
}

impl AxumWebhookServer {
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self {
            bind_addr,
            state: Arc::new(Mutex::new(WebhookServerState::Stopped)),
            shutdown_tx: Arc::new(Mutex::new(None)),
            local_addr: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn local_addr(&self) -> Option<SocketAddr> {
        *self.local_addr.lock().await
    }

    async fn set_state(&self, state: WebhookServerState) {
        *self.state.lock().await = state;
    }
}

#[async_trait]
impl WebhookServer for AxumWebhookServer {
    async fn run(&self, routes: Vec<WebhookRoute>) -> Result<()> {
        self.set_state(WebhookServerState::Starting).await;
        let listener = TcpListener::bind(self.bind_addr)
            .await
            .map_err(|err| AstrbotError::Platform(format!("bind webhook server: {err}")))?;
        let local_addr = listener
            .local_addr()
            .map_err(|err| AstrbotError::Platform(format!("read webhook server addr: {err}")))?;
        *self.local_addr.lock().await = Some(local_addr);
        let endpoint_count = routes.len();
        let routes = Arc::new(routes);
        let app = Router::new().fallback(webhook_fallback).with_state(routes);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        *self.shutdown_tx.lock().await = Some(shutdown_tx);
        self.set_state(WebhookServerState::Running { endpoint_count })
            .await;

        let result = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
        *self.shutdown_tx.lock().await = None;
        *self.local_addr.lock().await = None;
        self.set_state(WebhookServerState::Stopped).await;
        result.map_err(|err| AstrbotError::Platform(format!("run webhook server: {err}")))
    }

    async fn terminate(&self) -> Result<()> {
        self.set_state(WebhookServerState::Stopping).await;
        if let Some(sender) = self.shutdown_tx.lock().await.take() {
            let _ = sender.send(());
        }
        Ok(())
    }

    fn state(&self) -> WebhookServerState {
        self.state
            .try_lock()
            .map(|state| state.clone())
            .unwrap_or(WebhookServerState::Stopping)
    }
}

async fn webhook_fallback(
    State(routes): State<Arc<Vec<WebhookRoute>>>,
    request: Request<Body>,
) -> Response {
    let method = webhook_method(request.method());
    let path = request.uri().path().to_string();
    let query = parse_query(request.uri().query());
    let headers = request
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect::<Vec<_>>();
    let body = match to_bytes(request.into_body(), 1024 * 1024).await {
        Ok(body) => body.to_vec(),
        Err(error) => {
            return WebhookResponse::bad_request(format!("read webhook body: {error}"))
                .into_axum_response();
        }
    };
    let Some(route) = routes
        .iter()
        .find(|route| route.endpoint.path == path && route.endpoint.supports(&method))
    else {
        return (StatusCode::NOT_FOUND, "webhook route was not found").into_response();
    };

    let webhook_request = WebhookRequest {
        method,
        path,
        query,
        headers,
        body,
    };
    if let Some(verifier) = &route.signature_verifier
        && let Err(response) = verify_request_signature(verifier.as_ref(), &webhook_request)
    {
        return response.into_axum_response();
    }

    match route.handler.handle(webhook_request).await {
        Ok(response) => response.into_axum_response(),
        Err(error) => WebhookResponse::text(500, error.to_string()).into_axum_response(),
    }
}

impl WebhookResponse {
    fn into_axum_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut response = (status, self.body).into_response();
        if let Some(content_type) = self.content_type
            && let Ok(value) = content_type.parse()
        {
            response.headers_mut().insert(CONTENT_TYPE, value);
        }
        response
    }
}

fn verify_request_signature(
    verifier: &dyn WebhookSignatureVerifier,
    request: &WebhookRequest,
) -> std::result::Result<(), WebhookResponse> {
    let signature = request
        .query_value("msg_signature")
        .or_else(|| request.query_value("signature"))
        .or_else(|| request.header_value("x-line-signature"))
        .or_else(|| request.header_value("x-slack-signature"))
        .or_else(|| request.header_value("x-dingtalk-signature"))
        .or_else(|| request.header_value("x-signature"))
        .ok_or_else(|| WebhookResponse::text(401, "missing webhook signature"))?;
    let timestamp = request
        .query_value("timestamp")
        .or_else(|| request.query_value("ts"))
        .or_else(|| request.header_value("x-slack-request-timestamp"))
        .or_else(|| request.header_value("x-dingtalk-timestamp"))
        .or_else(|| request.header_value("x-timestamp"))
        .unwrap_or("0");
    let nonce = request
        .query_value("nonce")
        .or_else(|| request.header_value("x-nonce"))
        .unwrap_or("");
    let payload = String::from_utf8(request.body.clone())
        .map_err(|_| WebhookResponse::text(400, "webhook body must be UTF-8 for signature"))?;
    let input = WebhookSignatureInput::new(timestamp, nonce, payload);
    match verifier.verify(&input, signature) {
        Ok(WebhookSignatureVerdict::Match) => Ok(()),
        Ok(WebhookSignatureVerdict::Mismatch) => {
            Err(WebhookResponse::text(401, "webhook signature mismatch"))
        }
        Err(error) => Err(WebhookResponse::text(401, error.to_string())),
    }
}

fn webhook_method(method: &Method) -> WebhookHttpMethod {
    match *method {
        Method::GET => WebhookHttpMethod::Get,
        Method::PUT => WebhookHttpMethod::Put,
        Method::PATCH => WebhookHttpMethod::Patch,
        Method::DELETE => WebhookHttpMethod::Delete,
        _ => WebhookHttpMethod::Post,
    }
}

fn parse_query(query: Option<&str>) -> Vec<(String, String)> {
    query
        .unwrap_or_default()
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            (key.to_string(), value.to_string())
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebhookDuplicateStatus {
    New,
    Duplicate,
}

pub struct WebhookEventDeduplicator {
    ttl: Duration,
    seen: Mutex<HashMap<String, Instant>>,
}

impl WebhookEventDeduplicator {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            seen: Mutex::new(HashMap::new()),
        }
    }

    pub async fn check(&self, event_id: impl Into<String>) -> WebhookDuplicateStatus {
        let event_id = event_id.into();
        let now = Instant::now();
        let mut seen = self.seen.lock().await;
        seen.retain(|_, inserted_at| now.duration_since(*inserted_at) <= self.ttl);

        match seen.entry(event_id) {
            Entry::Occupied(_) => WebhookDuplicateStatus::Duplicate,
            Entry::Vacant(entry) => {
                entry.insert(now);
                WebhookDuplicateStatus::New
            }
        }
    }

    pub async fn len(&self) -> usize {
        self.seen.lock().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;

    use super::{
        AxumWebhookServer, WebhookCallbackHandler, WebhookDuplicateStatus, WebhookEndpoint,
        WebhookEventDeduplicator, WebhookHttpMethod, WebhookRequest, WebhookResponse, WebhookRoute,
        WebhookServer, WebhookServerState,
    };
    use crate::{Sha1SortedFieldsVerifier, WebhookSignatureInput, WebhookSignatureVerifier};

    #[test]
    fn endpoint_tracks_supported_callback_methods() {
        let endpoint = WebhookEndpoint::get("/callback").with_method(WebhookHttpMethod::Post);

        assert!(endpoint.supports(&WebhookHttpMethod::Get));
        assert!(endpoint.supports(&WebhookHttpMethod::Post));
        assert!(!endpoint.supports(&WebhookHttpMethod::Delete));
        assert_eq!(WebhookHttpMethod::Post.as_str(), "POST");
    }

    #[test]
    fn request_reads_query_and_headers_without_server_coupling() {
        let request = WebhookRequest::new(WebhookHttpMethod::Post, "/callback")
            .with_query("msg_signature", "sig")
            .with_header("Content-Type", "application/json")
            .with_body(br#"{"hello":"world"}"#.to_vec());

        assert_eq!(request.query_value("msg_signature"), Some("sig"));
        assert_eq!(
            request.header_value("content-type"),
            Some("application/json")
        );
        assert_eq!(request.body, br#"{"hello":"world"}"#.to_vec());
    }

    #[test]
    fn response_models_validation_and_acknowledgement_bodies() {
        let ok = WebhookResponse::ok_text("plain-token");
        let bad = WebhookResponse::bad_request("missing signature");

        assert_eq!(ok.status, 200);
        assert_eq!(
            ok.body_text().expect("response should be text"),
            "plain-token"
        );
        assert_eq!(bad.status, 400);
    }

    #[tokio::test]
    async fn deduplicator_detects_replayed_event_ids() {
        let deduplicator = WebhookEventDeduplicator::new(Duration::from_secs(60));

        assert_eq!(
            deduplicator.check("event-1").await,
            WebhookDuplicateStatus::New
        );
        assert_eq!(
            deduplicator.check("event-1").await,
            WebhookDuplicateStatus::Duplicate
        );
        assert_eq!(deduplicator.len().await, 1);
    }

    #[tokio::test]
    async fn axum_webhook_server_verifies_signature_and_shuts_down() {
        let handler = Arc::new(EchoWebhookHandler);
        let verifier = Arc::new(Sha1SortedFieldsVerifier::new("token").expect("verifier"));
        let route = WebhookRoute::new(WebhookEndpoint::post("/callback"), handler)
            .with_signature_verifier(verifier.clone());
        let server = Arc::new(AxumWebhookServer::new(
            "127.0.0.1:0".parse().expect("socket addr"),
        ));
        let running = server.clone();
        let task = tokio::spawn(async move { running.run(vec![route]).await });
        let address = wait_for_webhook_address(&server).await;
        let body = "payload";
        let signature = verifier
            .sign(&WebhookSignatureInput::new("1", "nonce", body))
            .expect("signature");
        let client = reqwest::Client::new();

        let response = client
            .post(format!(
                "http://{address}/callback?timestamp=1&nonce=nonce&msg_signature={signature}"
            ))
            .body(body.to_string())
            .send()
            .await
            .expect("webhook request");
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(response.text().await.expect("body"), "payload");

        let rejected = client
            .post(format!(
                "http://{address}/callback?timestamp=1&nonce=nonce&msg_signature=bad"
            ))
            .body(body.to_string())
            .send()
            .await
            .expect("webhook request");
        assert_eq!(rejected.status(), reqwest::StatusCode::UNAUTHORIZED);

        server.terminate().await.expect("terminate");
        task.await.expect("server join").expect("server result");
        assert_eq!(server.state(), WebhookServerState::Stopped);
    }

    struct EchoWebhookHandler;

    #[async_trait]
    impl WebhookCallbackHandler for EchoWebhookHandler {
        async fn handle(&self, request: WebhookRequest) -> astrbot_core::Result<WebhookResponse> {
            Ok(WebhookResponse::ok_text(
                String::from_utf8_lossy(&request.body).to_string(),
            ))
        }
    }

    async fn wait_for_webhook_address(server: &AxumWebhookServer) -> std::net::SocketAddr {
        for _ in 0..50 {
            if let Some(address) = server.local_addr().await {
                return address;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("webhook server did not start");
    }
}
