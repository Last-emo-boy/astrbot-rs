use std::sync::Arc;

use astrbot_core::{AstrbotError, MessageChain, MessageSession, MessageSink};
use tokio::sync::mpsc;

use crate::{RecordingSink, WebChatPlatform};

#[tokio::test]
async fn webchat_platform_submits_text_events() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = WebChatPlatform::with_identity("webchat", "WebChat", event_tx, sink);

    let event_id = platform
        .submit_text("conversation-1", "user-1", "hello webchat")
        .await
        .expect("webchat input should submit event");

    let event = event_rx.recv().await.expect("event should be queued");
    assert_eq!(event.id, event_id);
    assert_eq!(event.platform_id, "webchat");
    assert_eq!(event.sender.id, "user-1");
    assert_eq!(event.session.conversation_id, "conversation-1");
    assert_eq!(event.message.plain_text(), "hello webchat");
}

#[tokio::test]
async fn webchat_platform_submits_image_only_events() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = WebChatPlatform::with_identity("webchat", "WebChat", event_tx, sink);

    platform
        .submit_message(
            "conversation-1",
            "user-1",
            "",
            vec!["https://example.test/image.png".to_string()],
        )
        .await
        .expect("webchat image input should submit event");

    let event = event_rx.recv().await.expect("event should be queued");
    assert_eq!(event.platform_id, "webchat");
    assert_eq!(event.sender.id, "user-1");
    assert_eq!(event.session.conversation_id, "conversation-1");
    assert_eq!(event.message.plain_text(), "");
    assert_eq!(
        event.message.image_urls(),
        vec!["https://example.test/image.png".to_string()]
    );
}

#[tokio::test]
async fn webchat_platform_rejects_empty_message_chains() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = WebChatPlatform::with_identity("webchat", "WebChat", event_tx, sink);

    let result = platform
        .submit_chain("conversation-1", "user-1", MessageChain::default())
        .await;

    assert!(matches!(result, Err(AstrbotError::EmptyMessage)));
    assert!(event_rx.try_recv().is_err());
}

#[tokio::test]
async fn webchat_platform_filters_messages_by_conversation() {
    let (event_tx, _event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = WebChatPlatform::with_identity("webchat", "WebChat", event_tx, sink.clone());
    let session_a = MessageSession::new("webchat", "conversation-a");
    let session_b = MessageSession::new("webchat", "conversation-b");

    sink.send(&session_a, MessageChain::plain("alpha"))
        .await
        .expect("first message should record");
    sink.send(&session_b, MessageChain::plain("beta"))
        .await
        .expect("second message should record");

    let filtered = platform
        .sent_messages_for_conversation("conversation-a")
        .await;

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].session.conversation_id, "conversation-a");
    assert_eq!(filtered[0].chain.plain_text(), "alpha");
}
