use axum::{
    body::Body,
    http::{
        Request, StatusCode,
        header::{CONNECTION, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
    },
};
use serde_json::{Value, json};
use tower::ServiceExt;

use super::support::{get, post_json, post_multipart, response_json, router_for, webchat_fixture};

#[tokio::test]
async fn legacy_chat_routes_create_sessions_send_messages_and_upload_files() {
    let (webchat, mut event_rx) = webchat_fixture();
    let router = router_for(webchat);

    let created = get(router.clone(), "/api/chat/new_session").await;
    assert_eq!(created.status(), StatusCode::OK);
    let created: Value = response_json(created).await;
    let session_id = created["data"]["session_id"].as_str().expect("session id");

    let sent = post_json(
        router.clone(),
        "/api/chat/send",
        json!({
            "session_id": session_id,
            "sender_id": "user-1",
            "message": [
                { "type": "reply", "message_id": 42, "selected_text": "quoted" },
                { "type": "plain", "text": "hello source chat" },
                { "type": "image", "url": "https://example.test/a.png" }
            ],
            "selected_provider": "openai",
            "selected_model": "gpt-test",
            "enable_streaming": true
        }),
    )
    .await;
    assert_eq!(sent.status(), StatusCode::OK);
    let sent: Value = response_json(sent).await;
    assert_eq!(sent["status"], "ok");
    assert_eq!(sent["data"]["session_id"], session_id);

    let event = event_rx.recv().await.expect("chat event should enqueue");
    assert_eq!(event.session.conversation_id, session_id);
    assert_eq!(event.message.plain_text(), "hello source chat");
    assert_eq!(
        event.message.image_urls(),
        vec!["https://example.test/a.png".to_string()]
    );

    let sessions = get(router.clone(), "/api/chat/sessions").await;
    assert_eq!(sessions.status(), StatusCode::OK);
    let sessions: Value = response_json(sessions).await;
    assert!(
        sessions["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|session| { session["session_id"] == session_id })
    );

    let boundary = "astrbot-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"note.txt\"\r\nContent-Type: text/plain\r\n\r\nhello file\r\n--{boundary}--\r\n"
    )
    .into_bytes();
    let upload = post_multipart(router.clone(), "/api/chat/post_file", boundary, body).await;
    assert_eq!(upload.status(), StatusCode::OK);
    let upload: Value = response_json(upload).await;
    let attachment_id = upload["data"]["attachment_id"]
        .as_str()
        .expect("attachment id");

    let download = get(
        router,
        &format!("/api/chat/get_attachment?attachment_id={attachment_id}"),
    )
    .await;
    assert_eq!(download.status(), StatusCode::OK);
}

#[tokio::test]
async fn live_and_unified_chat_websocket_routes_upgrade_with_source_paths() {
    let (webchat, _event_rx) = webchat_fixture();
    let router = router_for(webchat);

    let missing_token = websocket_get(router.clone(), "/api/live_chat/ws").await;
    assert_eq!(missing_token.status(), StatusCode::UPGRADE_REQUIRED);

    let live = websocket_get(router.clone(), "/api/live_chat/ws?token=test-token").await;
    assert_eq!(live.status(), StatusCode::UPGRADE_REQUIRED);

    let unified = websocket_get(router, "/api/unified_chat/ws").await;
    assert_eq!(unified.status(), StatusCode::UPGRADE_REQUIRED);
}

async fn websocket_get(router: axum::Router, uri: &str) -> axum::response::Response {
    router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header(CONNECTION, "upgrade")
                .header(UPGRADE, "websocket")
                .header(SEC_WEBSOCKET_VERSION, "13")
                .header(SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
                .body(Body::empty())
                .expect("websocket request should build"),
        )
        .await
        .expect("router should respond")
}
