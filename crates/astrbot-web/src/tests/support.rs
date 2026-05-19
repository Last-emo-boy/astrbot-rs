use std::sync::Arc;

use astrbot_core::MessageEvent;
use astrbot_platform::{RecordingSink, WebChatPlatform};
use astrbot_runtime::RuntimeHandle;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, Response, header::CONTENT_TYPE},
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::mpsc;
use tower::ServiceExt;

use crate::webchat_router;

pub(super) fn webchat_fixture() -> (Arc<WebChatPlatform>, mpsc::Receiver<MessageEvent>) {
    let (event_tx, event_rx) = mpsc::channel(1);
    let webchat = Arc::new(WebChatPlatform::with_identity(
        "webchat",
        "WebChat",
        event_tx,
        Arc::new(RecordingSink::default()),
    ));
    (webchat, event_rx)
}

pub(super) fn router_for(webchat: Arc<WebChatPlatform>) -> Router {
    webchat_router(webchat)
}

pub(super) async fn post_json(router: Router, uri: &str, payload: Value) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

pub(super) async fn post_multipart(
    router: Router,
    uri: &str,
    boundary: &str,
    body: Vec<u8>,
) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(
                    CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

pub(super) async fn post_json_with_bearer(
    router: Router,
    uri: &str,
    payload: Value,
    token: &str,
) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(CONTENT_TYPE, "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

pub(super) async fn post_json_with_headers(
    router: Router,
    uri: &str,
    payload: Value,
    headers: &[(&str, &str)],
) -> Response<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header(CONTENT_TYPE, "application/json");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    router
        .oneshot(
            builder
                .body(Body::from(payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

pub(super) async fn put_json(router: Router, uri: &str, payload: Value) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(uri)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

pub(super) async fn patch_json(router: Router, uri: &str, payload: Value) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(uri)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

pub(super) async fn delete(router: Router, uri: &str) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

pub(super) async fn get(router: Router, uri: &str) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

pub(super) async fn get_with_bearer(router: Router, uri: &str, token: &str) -> Response<Body> {
    router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

pub(super) async fn response_json<T>(response: Response<Body>) -> T
where
    T: DeserializeOwned,
{
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should read");
    serde_json::from_slice(&body).expect("response JSON should parse")
}

pub(super) async fn wait_for_sent_messages(
    handle: &RuntimeHandle,
    platform_id: &str,
    expected: usize,
) -> Vec<astrbot_platform::SentMessage> {
    for _ in 0..64 {
        let sent = handle.sent_messages_for(platform_id).await;
        if sent.len() >= expected {
            return sent;
        }
        tokio::task::yield_now().await;
    }
    handle.sent_messages_for(platform_id).await
}
