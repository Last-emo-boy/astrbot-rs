use std::{
    fs,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use astrbot_storage::{ApiKeyRecord, ApiKeyRepository, InMemoryApiKeyRepository, SqliteStorage};
use axum::{
    Router,
    body::Body,
    http::{
        Request, Response, StatusCode,
        header::{CONNECTION, CONTENT_TYPE, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
    },
};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use tower::ServiceExt;

use crate::{
    ApiKeyIssuer, ManagementApiKeyState, OpenApiChatHttpResponse, OpenApiScope, OpenApiScopeSet,
    RealtimeControlState, hash_api_key, openapi_chat_router, openapi_chat_router_with_realtime,
};

use super::support::{response_json, webchat_fixture};

#[tokio::test]
async fn openapi_chat_route_requires_presented_api_key() {
    let (webchat, _event_rx) = webchat_fixture();
    let router = openapi_chat_router(webchat, Some(api_key_state()));

    let response = post_openapi_chat(router, None, chat_payload()).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn openapi_chat_route_requires_chat_scope() {
    let (webchat, _event_rx) = webchat_fixture();
    let repository = Arc::new(InMemoryApiKeyRepository::new());
    store_api_key(
        &repository,
        "key-1",
        "ak_management",
        [OpenApiScope::ManagementRead],
    )
    .await;
    let router = openapi_chat_router(webchat, Some(ManagementApiKeyState::new(repository)));

    let response = post_openapi_chat(router, Some("ak_management"), chat_payload()).await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn openapi_chat_route_rejects_expired_api_key() {
    let (webchat, _event_rx) = webchat_fixture();
    let repository = Arc::new(InMemoryApiKeyRepository::new());
    let secret = "ak_expired_openapi";
    repository
        .store_api_key(
            ApiKeyRecord::new(
                "key-expired",
                "Expired OpenAPI client",
                hash_api_key(secret),
                key_prefix(secret),
                ["chat"],
                "test",
            )
            .with_expires_at("unix:1"),
        )
        .await
        .expect("expired api key should store");
    let router = openapi_chat_router(webchat, Some(ManagementApiKeyState::new(repository)));

    let response = post_openapi_chat(router, Some(secret), chat_payload()).await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let payload: Value = response_json(response).await;
    assert_eq!(payload["error"], "openapi api key is expired");
}

#[tokio::test]
async fn openapi_chat_route_migrates_legacy_sha1_key_after_restart() {
    let db_path = std::env::temp_dir().join(format!(
        "astrbot-openapi-api-key-migration-{}.db",
        unique_nanos()
    ));
    let _ = fs::remove_file(&db_path);
    let secret = "ak_legacy_openapi";

    {
        let storage = SqliteStorage::open(&db_path).expect("sqlite storage should open");
        storage
            .store_api_key(ApiKeyRecord::new(
                "key-legacy",
                "Legacy OpenAPI client",
                legacy_sha1_hash(secret),
                key_prefix(secret),
                ["chat"],
                "test",
            ))
            .await
            .expect("legacy api key should store");
    }

    let (webchat, _event_rx) = webchat_fixture();
    let storage = Arc::new(SqliteStorage::open(&db_path).expect("sqlite storage should reopen"));
    let router = openapi_chat_router(webchat, Some(ManagementApiKeyState::new(storage)));

    let response = post_openapi_chat(router, Some(secret), chat_payload()).await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: OpenApiChatHttpResponse = response_json(response).await;
    assert_eq!(payload.key_id, "key-legacy");
    assert_eq!(payload.key_prefix, key_prefix(secret));

    let reopened = SqliteStorage::open(&db_path).expect("sqlite storage should reopen again");
    assert!(
        reopened
            .api_key_by_hash(&legacy_sha1_hash(secret))
            .await
            .expect("legacy hash lookup should run")
            .is_none()
    );
    let migrated = reopened
        .api_key_by_hash(&hash_api_key(secret))
        .await
        .expect("migrated hash lookup should run")
        .expect("migrated api key should exist");
    assert_eq!(migrated.key_id, "key-legacy");
    assert!(migrated.last_used_at.is_some());

    let _ = fs::remove_file(&db_path);
}

#[tokio::test]
async fn openapi_chat_route_enqueues_typed_message_parts() {
    let (webchat, mut event_rx) = webchat_fixture();
    let repository = Arc::new(InMemoryApiKeyRepository::new());
    store_api_key(&repository, "key-chat", "ak_openapi", [OpenApiScope::Chat]).await;
    let router = openapi_chat_router(webchat, Some(ManagementApiKeyState::new(repository)));

    let response = post_openapi_chat(
        router,
        Some("ak_openapi"),
        json!({
            "conversation_id": " conversation-1 ",
            "sender_id": " api-user ",
            "text": "hello openapi",
            "request_id": " request-1 ",
            "stream": true,
            "message_parts": [
                { "type": "image", "url": " https://example.com/a.png " },
                { "type": "plain", "text": " with text" }
            ]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: OpenApiChatHttpResponse = response_json(response).await;
    assert!(payload.accepted);
    assert_eq!(payload.conversation_id, "conversation-1");
    assert_eq!(payload.request_id.as_deref(), Some("request-1"));
    assert_eq!(payload.response_mode, "streaming");
    assert_eq!(
        payload.subscription.expect("subscription").request_id,
        "request-1"
    );
    assert_eq!(payload.key_id, "key-chat");
    assert_eq!(payload.key_prefix, "ak_openapi");

    let event = event_rx.recv().await.expect("event should enqueue");
    assert_eq!(event.id, payload.event_id);
    assert_eq!(event.session.conversation_id, "conversation-1");
    assert_eq!(event.sender.id, "api-user");
    assert_eq!(event.message.plain_text(), "hello openapi  with text");
    assert_eq!(
        event.message.image_urls(),
        vec!["https://example.com/a.png".to_string()]
    );
}

#[tokio::test]
async fn openapi_realtime_routes_track_subscription_and_stop_request() {
    let (webchat, _event_rx) = webchat_fixture();
    let realtime = RealtimeControlState::new();
    let repository = Arc::new(InMemoryApiKeyRepository::new());
    store_api_key(&repository, "key-chat", "ak_openapi", [OpenApiScope::Chat]).await;
    let router = openapi_chat_router_with_realtime(
        webchat,
        Some(ManagementApiKeyState::new(repository)),
        realtime.clone(),
    );

    let response = post_openapi_chat(
        router.clone(),
        Some("ak_openapi"),
        json!({
            "conversation_id": "conversation-1",
            "sender_id": "api-user",
            "text": "hello stream",
            "request_id": "request-1",
            "stream": true
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: OpenApiChatHttpResponse = response_json(response).await;
    let event_id = payload.event_id.clone();

    let status = get_openapi(
        router.clone(),
        Some("ak_openapi"),
        "/api/openapi/chat/subscriptions/request-1",
    )
    .await;
    assert_eq!(status.status(), StatusCode::OK);
    let status: Value = response_json(status).await;
    assert_eq!(status["status"], "queued");
    assert_eq!(status["event_id"], event_id);

    let stop = post_openapi(
        router.clone(),
        Some("ak_openapi"),
        "/api/openapi/chat/stop",
        json!({ "conversation_id": "conversation-1", "request_id": "request-1" }),
    )
    .await;
    assert_eq!(stop.status(), StatusCode::OK);
    let stop: Value = response_json(stop).await;
    assert_eq!(stop["matched_subscriptions"], 1);
    assert_eq!(stop["interrupted_events"], 1);
    assert_eq!(stop["status"], "stop_requested");

    let active = realtime
        .active_event_record(&event_id)
        .expect("active event lookup")
        .expect("active event");
    assert!(active.agent_stop_requested);
}

#[tokio::test]
async fn openapi_elicitation_routes_create_and_respond_to_typed_requests() {
    let (webchat, _event_rx) = webchat_fixture();
    let repository = Arc::new(InMemoryApiKeyRepository::new());
    store_api_key(&repository, "key-chat", "ak_openapi", [OpenApiScope::Chat]).await;
    let router = openapi_chat_router(webchat, Some(ManagementApiKeyState::new(repository)));

    let created = post_openapi(
        router.clone(),
        Some("ak_openapi"),
        "/api/openapi/elicitation",
        json!({
            "elicitation_id": "approval-1",
            "conversation_id": "conversation-1",
            "request_id": "request-1",
            "request": {
                "kind": "form",
                "message": "Approve action?",
                "requested_schema": {
                    "properties": {
                        "confirmed": { "type": "boolean" }
                    },
                    "required": ["confirmed"]
                }
            }
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let created: Value = response_json(created).await;
    assert_eq!(created["status"], "pending");
    assert_eq!(created["request"]["kind"], "form");

    let responded = post_openapi(
        router,
        Some("ak_openapi"),
        "/api/openapi/elicitation/respond",
        json!({
            "elicitation_id": "approval-1",
            "result": {
                "action": "accept",
                "content": { "confirmed": true }
            }
        }),
    )
    .await;
    assert_eq!(responded.status(), StatusCode::OK);
    let responded: Value = response_json(responded).await;
    assert_eq!(responded["status"], "responded");
    assert_eq!(responded["result"]["action"], "accept");
    assert_eq!(responded["result"]["content"]["confirmed"], true);
}

#[tokio::test]
async fn openapi_v1_chat_websocket_requires_chat_key_and_upgrades() {
    let (webchat, _event_rx) = webchat_fixture();
    let repository = Arc::new(InMemoryApiKeyRepository::new());
    store_api_key(&repository, "key-chat", "ak_openapi", [OpenApiScope::Chat]).await;
    let router = openapi_chat_router(webchat, Some(ManagementApiKeyState::new(repository)));

    let sessions = get_openapi(router.clone(), None, "/api/v1/chat/sessions").await;
    assert_eq!(sessions.status(), StatusCode::UNAUTHORIZED);

    let unauthorized = websocket_get_openapi(router.clone(), None, "/api/v1/chat/ws").await;
    assert_eq!(unauthorized.status(), StatusCode::UPGRADE_REQUIRED);

    let upgraded = websocket_get_openapi(router, Some("ak_openapi"), "/api/v1/chat/ws").await;
    assert_eq!(upgraded.status(), StatusCode::UPGRADE_REQUIRED);
}

fn api_key_state() -> ManagementApiKeyState {
    ManagementApiKeyState::new(Arc::new(InMemoryApiKeyRepository::new()))
}

async fn store_api_key(
    repository: &Arc<InMemoryApiKeyRepository>,
    key_id: &str,
    secret: &str,
    scopes: impl IntoIterator<Item = OpenApiScope>,
) {
    let issued = ApiKeyIssuer::issue(
        key_id,
        "OpenAPI client",
        secret,
        OpenApiScopeSet::new(scopes),
        "test",
    );
    repository
        .store_api_key(issued.record)
        .await
        .expect("api key should store");
}

fn key_prefix(secret: &str) -> String {
    secret.chars().take(12).collect()
}

fn legacy_sha1_hash(secret: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn unique_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn chat_payload() -> Value {
    json!({
        "conversation_id": "conversation-1",
        "sender_id": "api-user",
        "text": "hello"
    })
}

async fn post_openapi_chat(router: Router, secret: Option<&str>, payload: Value) -> Response<Body> {
    post_openapi(router, secret, "/api/openapi/chat", payload).await
}

async fn post_openapi(
    router: Router,
    secret: Option<&str>,
    path: &str,
    payload: Value,
) -> Response<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header(CONTENT_TYPE, "application/json");
    if let Some(secret) = secret {
        builder = builder.header("authorization", format!("Bearer {secret}"));
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

async fn get_openapi(router: Router, secret: Option<&str>, path: &str) -> Response<Body> {
    let mut builder = Request::builder().method("GET").uri(path);
    if let Some(secret) = secret {
        builder = builder.header("authorization", format!("Bearer {secret}"));
    }

    router
        .oneshot(builder.body(Body::empty()).expect("request should build"))
        .await
        .expect("router should respond")
}

async fn websocket_get_openapi(router: Router, secret: Option<&str>, path: &str) -> Response<Body> {
    let mut builder = Request::builder()
        .method("GET")
        .uri(path)
        .header(CONNECTION, "upgrade")
        .header(UPGRADE, "websocket")
        .header(SEC_WEBSOCKET_VERSION, "13")
        .header(SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==");
    if let Some(secret) = secret {
        builder = builder.header("authorization", format!("Bearer {secret}"));
    }

    router
        .oneshot(builder.body(Body::empty()).expect("request should build"))
        .await
        .expect("router should respond")
}
