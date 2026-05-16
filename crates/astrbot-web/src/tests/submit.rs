use axum::http::StatusCode;
use serde_json::json;

use crate::SubmitTextResponse;

use super::support::{post_json, response_json, router_for, webchat_fixture};

#[tokio::test]
async fn post_webchat_message_submits_platform_event() {
    let (webchat, mut event_rx) = webchat_fixture();
    let router = router_for(webchat);

    let response = post_json(
        router,
        "/api/webchat/conversation-1",
        json!({
            "sender_id": "user-1",
            "text": "hello",
            "image_urls": ["https://example.test/image.png"]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: SubmitTextResponse = response_json(response).await;
    assert!(!payload.event_id.is_empty());

    let event = event_rx.recv().await.expect("event should be submitted");
    assert_eq!(event.id, payload.event_id);
    assert_eq!(event.platform_id, "webchat");
    assert_eq!(event.session.conversation_id, "conversation-1");
    assert_eq!(event.sender.id, "user-1");
    assert_eq!(event.message.plain_text(), "hello");
    assert_eq!(
        event.message.image_urls(),
        vec!["https://example.test/image.png".to_string()]
    );
}

#[tokio::test]
async fn image_only_webchat_message_is_accepted() {
    let (webchat, mut event_rx) = webchat_fixture();
    let router = router_for(webchat);

    let response = post_json(
        router,
        "/api/webchat/conversation-1",
        json!({
            "sender_id": "user-1",
            "text": "",
            "image_urls": ["https://example.test/image-only.png"]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: SubmitTextResponse = response_json(response).await;
    assert!(!payload.event_id.is_empty());

    let event = event_rx.recv().await.expect("event should be submitted");
    assert_eq!(event.id, payload.event_id);
    assert_eq!(event.message.plain_text(), "");
    assert_eq!(
        event.message.image_urls(),
        vec!["https://example.test/image-only.png".to_string()]
    );
}

#[tokio::test]
async fn message_parts_webchat_message_is_accepted() {
    let (webchat, mut event_rx) = webchat_fixture();
    let router = router_for(webchat);

    let response = post_json(
        router,
        "/api/webchat/conversation-1",
        json!({
            "sender_id": "user-1",
            "message_parts": [
                {"type": "plain", "text": "hello from parts"},
                {"type": "image", "url": "https://example.test/part.png"}
            ]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: SubmitTextResponse = response_json(response).await;

    let event = event_rx.recv().await.expect("event should be submitted");
    assert_eq!(event.id, payload.event_id);
    assert_eq!(event.message.plain_text(), "hello from parts");
    assert_eq!(
        event.message.image_urls(),
        vec!["https://example.test/part.png".to_string()]
    );
}
