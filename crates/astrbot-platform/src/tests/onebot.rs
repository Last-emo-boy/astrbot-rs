use std::sync::Arc;

use astrbot_core::{AstrbotError, MessageChain};
use tokio::sync::mpsc;

use crate::{OneBotPlatform, RecordingSink};

#[tokio::test]
async fn onebot_platform_submits_private_text_events() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = OneBotPlatform::with_identity("onebot", "OneBot", event_tx, sink);

    let event_id = platform
        .submit_private_text("user-1", "hello onebot")
        .await
        .expect("onebot private input should submit event");

    let event = event_rx.recv().await.expect("event should be queued");
    assert_eq!(event.id, event_id);
    assert_eq!(event.platform_id, "onebot");
    assert_eq!(event.platform_name, "OneBot");
    assert_eq!(event.sender.id, "user-1");
    assert!(event.session.is_direct());
    assert_eq!(event.session.conversation_id, "private:user-1");
    assert_eq!(event.message.plain_text(), "hello onebot");
}

#[tokio::test]
async fn onebot_platform_submits_group_text_events() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = OneBotPlatform::with_identity("onebot", "OneBot", event_tx, sink);

    let event_id = platform
        .submit_group_text("group-1", "user-1", "hello group")
        .await
        .expect("onebot group input should submit event");

    let event = event_rx.recv().await.expect("event should be queued");
    assert_eq!(event.id, event_id);
    assert_eq!(event.platform_id, "onebot");
    assert_eq!(event.sender.id, "user-1");
    assert!(event.session.is_group());
    assert_eq!(event.session.conversation_id, "group:group-1");
    assert_eq!(
        event.identity().and_then(|identity| identity.group_id()),
        Some("group-1")
    );
    assert_eq!(event.message.plain_text(), "hello group");
}

#[tokio::test]
async fn onebot_platform_rejects_empty_message_chains() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = OneBotPlatform::with_identity("onebot", "OneBot", event_tx, sink);

    let result = platform
        .submit_private_chain("user-1", MessageChain::default())
        .await;

    assert!(matches!(result, Err(AstrbotError::EmptyMessage)));
    assert!(event_rx.try_recv().is_err());
}
