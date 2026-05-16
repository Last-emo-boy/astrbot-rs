use astrbot_core::{MessageChain, MessageComponent, MessageSession, MessageSink};
use axum::http::StatusCode;

use crate::{WebChatMessagePart, WebChatMessageResponse, WebChatMessagesResponse};

use super::support::{get, response_json, router_for, webchat_fixture};

#[tokio::test]
async fn get_webchat_messages_returns_recorded_history() {
    let (webchat, _event_rx) = webchat_fixture();
    let sink = webchat.sink();
    sink.send(
        &MessageSession::new("webchat", "conversation-1"),
        MessageChain::new(vec![
            MessageComponent::reply("source-1", "quoted reply"),
            MessageComponent::plain("first reply"),
            MessageComponent::image("https://example.test/reply.png"),
            MessageComponent::record("https://example.test/reply.ogg"),
            MessageComponent::video("https://example.test/reply.mp4"),
            MessageComponent::file("reply.txt", "https://example.test/reply.txt"),
        ]),
    )
    .await
    .expect("first reply should record");
    sink.send(
        &MessageSession::new("webchat", "conversation-2"),
        MessageChain::plain("other reply"),
    )
    .await
    .expect("other reply should record");
    let router = router_for(webchat);

    let response = get(router, "/api/webchat/conversation-1/messages").await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: WebChatMessagesResponse = response_json(response).await;
    assert_eq!(payload.conversation_id, "conversation-1");
    assert_eq!(
        payload.messages,
        vec![WebChatMessageResponse {
            text: "first reply".to_string(),
            image_urls: vec!["https://example.test/reply.png".to_string()],
            message_parts: vec![
                WebChatMessagePart::Reply {
                    message_id: "source-1".to_string(),
                    selected_text: "quoted reply".to_string(),
                },
                WebChatMessagePart::Plain {
                    text: "first reply".to_string(),
                },
                WebChatMessagePart::Image {
                    url: "https://example.test/reply.png".to_string(),
                },
                WebChatMessagePart::Record {
                    url: "https://example.test/reply.ogg".to_string(),
                },
                WebChatMessagePart::Video {
                    url: "https://example.test/reply.mp4".to_string(),
                },
                WebChatMessagePart::File {
                    name: "reply.txt".to_string(),
                    url: "https://example.test/reply.txt".to_string(),
                },
            ],
        }]
    );
}
