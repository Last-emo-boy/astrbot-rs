use astrbot_core::{MessageChain, MessageSession, MessageSink, MessageStream};

use crate::RecordingSink;

#[tokio::test]
async fn recording_sink_records_streaming_messages_separately() {
    let sink = RecordingSink::default();
    let session = MessageSession::new("mock", "conversation-1");

    sink.send_streaming(
        &session,
        MessageStream::new(vec![MessageChain::plain("one"), MessageChain::plain("two")]),
    )
    .await
    .expect("streaming message should record");

    assert!(sink.messages().await.is_empty());
    let streamed = sink.streaming_messages().await;
    assert_eq!(streamed.len(), 1);
    assert_eq!(streamed[0].session, session);
    assert_eq!(streamed[0].stream.chunks()[0].plain_text(), "one");
    assert_eq!(streamed[0].stream.chunks()[1].plain_text(), "two");
}
