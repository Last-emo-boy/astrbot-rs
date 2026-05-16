use std::collections::{HashMap, hash_map::Entry};
use std::sync::Arc;
use std::time::{Duration, Instant};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use tokio::sync::Mutex;

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
}

impl WebhookRoute {
    pub fn new(endpoint: WebhookEndpoint, handler: Arc<dyn WebhookCallbackHandler>) -> Self {
        Self { endpoint, handler }
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
    use std::time::Duration;

    use super::{
        WebhookDuplicateStatus, WebhookEndpoint, WebhookEventDeduplicator, WebhookHttpMethod,
        WebhookRequest, WebhookResponse,
    };

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
}
