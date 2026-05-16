use astrbot_core::MessageComponent;
use axum::http::StatusCode;
use serde_json::json;

use crate::ErrorResponse;

use super::support::{post_json, response_json, router_for, webchat_fixture};

#[tokio::test]
async fn reply_message_part_is_preserved_with_content() {
    let (webchat, mut event_rx) = webchat_fixture();
    let router = router_for(webchat);

    let response = post_json(
        router,
        "/api/webchat/conversation-1",
        json!({
            "sender_id": "user-1",
            "message_parts": [
                {
                    "type": "reply",
                    "message_id": 42,
                    "selected_text": "quoted text"
                },
                {"type": "plain", "text": "answer"}
            ]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let event = event_rx.recv().await.expect("event should be submitted");
    assert_eq!(
        event.message.components(),
        &[
            MessageComponent::reply("42", "quoted text"),
            MessageComponent::plain("answer"),
        ]
    );
    assert_eq!(event.message.plain_text(), "answer");
}

#[tokio::test]
async fn reply_only_webchat_message_returns_bad_request() {
    let (webchat, mut event_rx) = webchat_fixture();
    let router = router_for(webchat);

    let response = post_json(
        router,
        "/api/webchat/conversation-1",
        json!({
            "sender_id": "user-1",
            "message_parts": [
                {
                    "type": "reply",
                    "message_id": "message-1",
                    "selected_text": "quoted text"
                }
            ]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(event_rx.try_recv().is_err());
}

#[tokio::test]
async fn non_image_media_message_parts_are_preserved() {
    let (webchat, mut event_rx) = webchat_fixture();
    let router = router_for(webchat);

    let response = post_json(
        router,
        "/api/webchat/conversation-1",
        json!({
            "sender_id": "user-1",
            "message_parts": [
                {"type": "record", "url": "https://example.test/audio.ogg"},
                {"type": "video", "url": "https://example.test/video.mp4"},
                {
                    "type": "file",
                    "name": "report.pdf",
                    "url": "https://example.test/report.pdf"
                }
            ]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let event = event_rx.recv().await.expect("event should be submitted");
    assert_eq!(
        event.message.components(),
        &[
            MessageComponent::record("https://example.test/audio.ogg"),
            MessageComponent::video("https://example.test/video.mp4"),
            MessageComponent::file("report.pdf", "https://example.test/report.pdf"),
        ]
    );
    assert_eq!(event.message.plain_text(), "");
    assert!(event.message.image_urls().is_empty());
}

#[tokio::test]
async fn empty_webchat_message_returns_bad_request() {
    let (webchat, mut event_rx) = webchat_fixture();
    let router = router_for(webchat);

    let response = post_json(
        router,
        "/api/webchat/conversation-1",
        json!({"sender_id": "user-1", "text": ""}),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload: ErrorResponse = response_json(response).await;
    assert_eq!(payload.error, "message is empty");
    assert!(event_rx.try_recv().is_err());
}
