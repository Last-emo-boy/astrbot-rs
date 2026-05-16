use std::sync::Arc;

use astrbot_core::{MessageChain, MessageSession, MessageSink};
use tokio::sync::mpsc;

use crate::{ConsolePlatform, ConsoleSink};

#[tokio::test]
async fn console_platform_parses_input_into_events() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(ConsoleSink::default());
    let platform = ConsolePlatform::with_identity("console", "Console", event_tx, sink);

    assert!(
        platform
            .handle_line("alice: hello from console")
            .await
            .expect("console line should be handled")
    );

    let event = event_rx.recv().await.expect("event should be queued");
    assert_eq!(event.platform_id, "console");
    assert_eq!(event.sender.id, "alice");
    assert_eq!(event.session.conversation_id, "console");
    assert_eq!(event.message.plain_text(), "hello from console");
}

#[tokio::test]
async fn console_sink_records_sent_messages() {
    let sink = ConsoleSink::default();
    let session = MessageSession::new("console", "console");

    sink.send(&session, MessageChain::plain("response"))
        .await
        .expect("console sink should record output");

    let sent = sink.messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "response");
}
