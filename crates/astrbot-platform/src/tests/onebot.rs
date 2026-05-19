use std::sync::Arc;

use astrbot_core::{AstrbotError, MessageChain};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{OneBotPlatform, OneBotTransport, PlatformAdapter, RecordingSink};

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

#[tokio::test]
async fn onebot_reverse_websocket_handles_auth_inbound_outbound_reconnect_and_shutdown() {
    let port = free_local_port();
    let (event_tx, mut event_rx) = mpsc::channel(4);
    let sink = Arc::new(RecordingSink::default());
    let platform = Arc::new(
        OneBotPlatform::with_identity("onebot", "OneBot", event_tx, sink).with_transport(
            OneBotTransport::reverse_websocket_with_token(
                "127.0.0.1",
                port,
                Some("secret".to_string()),
            ),
        ),
    );
    let runner_platform = platform.clone();
    let runner = tokio::spawn(async move { runner_platform.run().await });
    wait_for_ws(&format!("ws://127.0.0.1:{port}/?access_token=secret")).await;

    assert!(
        connect_async(format!("ws://127.0.0.1:{port}/"))
            .await
            .is_err()
    );

    let (mut ws, _) = connect_async(format!("ws://127.0.0.1:{port}/?access_token=secret"))
        .await
        .expect("authorized onebot ws should connect");
    ws.send(Message::Text(
        json!({
            "post_type": "message",
            "message_type": "private",
            "self_id": 42,
            "user_id": 1001,
            "message_id": 9001,
            "message": [
                {"type": "text", "data": {"text": "hello"}},
                {"type": "image", "data": {"url": "https://example.test/a.png"}}
            ]
        })
        .to_string()
        .into(),
    ))
    .await
    .expect("fake client should send event");

    let event = event_rx
        .recv()
        .await
        .expect("onebot event should be queued");
    assert_eq!(event.id, "onebot-event-9001");
    assert_eq!(event.sender.id, "1001");
    assert!(event.session.is_direct());
    assert_eq!(event.session.conversation_id, "private:1001");
    assert_eq!(event.self_id(), Some("42"));
    assert_eq!(event.message.plain_text(), "hello");
    assert_eq!(
        event.message.image_urls(),
        vec!["https://example.test/a.png"]
    );

    event
        .send(MessageChain::plain("reply"))
        .await
        .expect("reply should emit onebot action");
    let action = next_json_message(&mut ws).await;
    assert_eq!(action["action"], "send_private_msg");
    assert_eq!(action["params"]["user_id"], 1001);
    assert_eq!(action["params"]["message"][0]["data"]["text"], "reply");

    ws.close(None).await.expect("client should close");

    let (mut ws2, _) = connect_async(format!("ws://127.0.0.1:{port}/?access_token=secret"))
        .await
        .expect("fake client should reconnect");
    ws2.send(Message::Text(
        json!({
            "post_type": "message",
            "message_type": "group",
            "user_id": 1002,
            "group_id": 2001,
            "message_id": 9002,
            "message": [
                {"type": "text", "data": {"text": "group hello"}},
                {"type": "record", "data": {"file": "voice.amr"}}
            ]
        })
        .to_string()
        .into(),
    ))
    .await
    .expect("fake client should send group event");
    let group_event = event_rx.recv().await.expect("group event should be queued");
    assert!(group_event.session.is_group());
    assert_eq!(group_event.session.conversation_id, "group:2001");
    assert_eq!(group_event.sender.id, "1002");
    assert_eq!(group_event.message.plain_text(), "group hello");

    platform
        .terminate()
        .await
        .expect("onebot platform should terminate");
    timeout(Duration::from_secs(3), runner)
        .await
        .expect("onebot runner should stop")
        .expect("onebot runner should join")
        .expect("onebot runner should succeed");
}

fn free_local_port() -> u16 {
    let listener =
        std::net::TcpListener::bind(("127.0.0.1", 0)).expect("free port listener should bind");
    listener
        .local_addr()
        .expect("local addr should exist")
        .port()
}

async fn wait_for_ws(url: &str) {
    for _ in 0..64 {
        match connect_async(url.to_string()).await {
            Ok((mut ws, _)) => {
                let _ = ws.close(None).await;
                return;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(25)).await,
        }
    }
    panic!("onebot ws server did not start");
}

async fn next_json_message(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Value {
    let message = timeout(Duration::from_secs(3), ws.next())
        .await
        .expect("websocket message should arrive")
        .expect("websocket should still be open")
        .expect("websocket message should be ok");
    let Message::Text(text) = message else {
        panic!("expected websocket text message");
    };
    serde_json::from_str(&text).expect("websocket action should be json")
}
