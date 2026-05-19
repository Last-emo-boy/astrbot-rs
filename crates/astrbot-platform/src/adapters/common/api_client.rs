use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::Method;
use tokio::time;

use super::retry::{PlatformRetryPolicy, PlatformRetryReason};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformApiMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    WebSocketConnect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformApiRequest {
    pub platform_type: String,
    pub method: PlatformApiMethod,
    pub endpoint: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub rate_limit_key: Option<String>,
}

impl PlatformApiRequest {
    pub fn new(
        platform_type: impl Into<String>,
        method: PlatformApiMethod,
        endpoint: impl Into<String>,
    ) -> Self {
        Self {
            platform_type: platform_type.into(),
            method,
            endpoint: endpoint.into(),
            headers: Vec::new(),
            body: Vec::new(),
            rate_limit_key: None,
        }
    }

    pub fn websocket(platform_type: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self::new(platform_type, PlatformApiMethod::WebSocketConnect, endpoint)
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    pub fn with_rate_limit_key(mut self, rate_limit_key: impl Into<String>) -> Self {
        let rate_limit_key = rate_limit_key.into();
        self.rate_limit_key = (!rate_limit_key.trim().is_empty()).then_some(rate_limit_key);
        self
    }

    pub fn is_websocket(&self) -> bool {
        self.method == PlatformApiMethod::WebSocketConnect
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformApiResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl PlatformApiResponse {
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn retry_after(&self) -> Option<Duration> {
        self.headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
            .and_then(|(_, value)| value.parse::<u64>().ok())
            .map(Duration::from_secs)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformApiErrorKind {
    Connection,
    Authentication,
    RateLimited,
    NotFound,
    Server,
    WebSocket,
    InvalidResponse,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformApiError {
    pub platform_type: String,
    pub endpoint: String,
    pub kind: PlatformApiErrorKind,
    pub status: Option<u16>,
    pub retry_after: Option<Duration>,
    pub message: String,
}

impl PlatformApiError {
    pub fn new(
        platform_type: impl Into<String>,
        endpoint: impl Into<String>,
        kind: PlatformApiErrorKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            platform_type: platform_type.into(),
            endpoint: endpoint.into(),
            kind,
            status: None,
            retry_after: None,
            message: message.into(),
        }
    }

    pub fn from_status(
        platform_type: impl Into<String>,
        endpoint: impl Into<String>,
        status: u16,
    ) -> Self {
        let kind = match status {
            401 | 403 => PlatformApiErrorKind::Authentication,
            404 => PlatformApiErrorKind::NotFound,
            429 => PlatformApiErrorKind::RateLimited,
            500..=599 => PlatformApiErrorKind::Server,
            _ => PlatformApiErrorKind::Unknown,
        };
        Self {
            platform_type: platform_type.into(),
            endpoint: endpoint.into(),
            kind,
            status: Some(status),
            retry_after: None,
            message: format!("platform API returned HTTP {status}"),
        }
    }

    pub fn websocket(
        platform_type: impl Into<String>,
        endpoint: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            platform_type,
            endpoint,
            PlatformApiErrorKind::WebSocket,
            message,
        )
    }

    pub fn with_retry_after(mut self, retry_after: Duration) -> Self {
        self.retry_after = Some(retry_after);
        self
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self.kind,
            PlatformApiErrorKind::Connection
                | PlatformApiErrorKind::RateLimited
                | PlatformApiErrorKind::Server
                | PlatformApiErrorKind::WebSocket
        )
    }
}

impl From<PlatformApiError> for AstrbotError {
    fn from(error: PlatformApiError) -> Self {
        let status = error
            .status
            .map(|status| format!(" HTTP {status}"))
            .unwrap_or_default();
        AstrbotError::Platform(format!(
            "{} API {:?}{} at {}: {}",
            error.platform_type, error.kind, status, error.endpoint, error.message
        ))
    }
}

#[async_trait]
pub trait PlatformApiClient: Send + Sync {
    async fn execute(&self, request: PlatformApiRequest) -> Result<PlatformApiResponse>;
}

#[derive(Clone, Debug)]
pub struct ReqwestPlatformApiClient {
    client: reqwest::Client,
    retry_policy: PlatformRetryPolicy,
}

impl ReqwestPlatformApiClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            retry_policy: PlatformRetryPolicy::default(),
        }
    }

    pub fn with_retry_policy(mut self, retry_policy: PlatformRetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    async fn execute_once(&self, request: &PlatformApiRequest) -> Result<PlatformApiResponse> {
        let method = reqwest_method(request.method)?;
        let mut builder = self.client.request(method, &request.endpoint);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if !request.body.is_empty() {
            builder = builder.body(request.body.clone());
        }

        let response = builder.send().await.map_err(|err| {
            PlatformApiError::new(
                &request.platform_type,
                &request.endpoint,
                PlatformApiErrorKind::Connection,
                err.to_string(),
            )
        })?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_string(), value.to_string()))
            })
            .collect::<Vec<_>>();
        let body = response
            .bytes()
            .await
            .map_err(|err| {
                PlatformApiError::new(
                    &request.platform_type,
                    &request.endpoint,
                    PlatformApiErrorKind::InvalidResponse,
                    err.to_string(),
                )
            })?
            .to_vec();

        Ok(PlatformApiResponse {
            status,
            headers,
            body,
        })
    }
}

impl Default for ReqwestPlatformApiClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformApiClient for ReqwestPlatformApiClient {
    async fn execute(&self, request: PlatformApiRequest) -> Result<PlatformApiResponse> {
        if request.is_websocket() {
            return Err(PlatformApiError::websocket(
                &request.platform_type,
                &request.endpoint,
                "websocket requests must use LongConnectionClient",
            )
            .into());
        }

        let mut attempt = 1;
        loop {
            let response = self.execute_once(&request).await;
            match response {
                Ok(response) if response.is_success() => return Ok(response),
                Ok(response) => {
                    let mut error = PlatformApiError::from_status(
                        &request.platform_type,
                        &request.endpoint,
                        response.status,
                    );
                    if let Some(retry_after) = response.retry_after() {
                        error = error.with_retry_after(retry_after);
                    }
                    if let Some(reason) = PlatformRetryReason::from_api_error_kind(error.kind)
                        && let Some(policy_delay) =
                            self.retry_policy.delay_after_failure(attempt, reason)
                    {
                        let delay = error.retry_after.unwrap_or(policy_delay);
                        time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(error.into());
                }
                Err(error) => {
                    let api_error = PlatformApiError::new(
                        &request.platform_type,
                        &request.endpoint,
                        PlatformApiErrorKind::Connection,
                        error.to_string(),
                    );
                    if let Some(reason) = PlatformRetryReason::from_api_error_kind(api_error.kind)
                        && let Some(delay) = self.retry_policy.delay_after_failure(attempt, reason)
                    {
                        time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(api_error.into());
                }
            }
        }
    }
}

fn reqwest_method(method: PlatformApiMethod) -> Result<Method> {
    match method {
        PlatformApiMethod::Get => Ok(Method::GET),
        PlatformApiMethod::Post => Ok(Method::POST),
        PlatformApiMethod::Put => Ok(Method::PUT),
        PlatformApiMethod::Patch => Ok(Method::PATCH),
        PlatformApiMethod::Delete => Ok(Method::DELETE),
        PlatformApiMethod::WebSocketConnect => Err(PlatformApiError::websocket(
            "platform",
            "websocket",
            "websocket method is not an HTTP API request",
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use axum::{Router, routing::post};

    use super::{
        PlatformApiClient, PlatformApiError, PlatformApiErrorKind, PlatformApiMethod,
        PlatformApiRequest, PlatformApiResponse, ReqwestPlatformApiClient,
    };
    use crate::PlatformRetryPolicy;

    #[test]
    fn api_request_keeps_client_details_outside_event_conversion() {
        let request = PlatformApiRequest::new("misskey", PlatformApiMethod::Post, "notes/create")
            .with_header("authorization", "Bearer token")
            .with_rate_limit_key("notes");

        assert_eq!(request.platform_type, "misskey");
        assert_eq!(request.method, PlatformApiMethod::Post);
        assert_eq!(request.rate_limit_key.as_deref(), Some("notes"));
        assert!(!request.is_websocket());
    }

    #[test]
    fn response_and_error_classify_rate_limits_and_websocket_failures() {
        let response = PlatformApiResponse::new(429).with_header("Retry-After", "3");
        let error = PlatformApiError::from_status("misskey", "notes/create", response.status)
            .with_retry_after(response.retry_after().expect("retry-after header"));
        let websocket = PlatformApiError::websocket("misskey", "streaming", "connection closed");

        assert_eq!(error.kind, PlatformApiErrorKind::RateLimited);
        assert!(error.is_retryable());
        assert_eq!(error.retry_after.expect("retry after").as_secs(), 3);
        assert_eq!(websocket.kind, PlatformApiErrorKind::WebSocket);
        assert!(websocket.is_retryable());
    }

    #[tokio::test]
    async fn reqwest_api_client_retries_429_and_returns_success_body() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let state = attempts.clone();
        let app = Router::new().route(
            "/send",
            post(move || {
                let state = state.clone();
                async move {
                    let attempt = state.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        (
                            axum::http::StatusCode::TOO_MANY_REQUESTS,
                            [("Retry-After", "0")],
                            "rate limited",
                        )
                    } else {
                        (axum::http::StatusCode::OK, [("Retry-After", "0")], "ok")
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("local addr");
        let server = tokio::spawn(async move { axum::serve(listener, app).await });

        let client = ReqwestPlatformApiClient::new().with_retry_policy(PlatformRetryPolicy::new(
            2,
            Duration::from_millis(1),
            Duration::from_millis(1),
        ));
        let response = client
            .execute(PlatformApiRequest::new(
                "fake",
                PlatformApiMethod::Post,
                format!("http://{address}/send"),
            ))
            .await
            .expect("request should retry and succeed");

        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"ok");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        server.abort();
    }
}
