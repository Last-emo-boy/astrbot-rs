use astrbot_runtime::{AstrbotRuntime, RuntimeConfig, RuntimePlatformConfig};
use axum::http::StatusCode;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::{
    SubmitTextRequest, SubmitTextResponse, WebChatMessagePart, WebChatMessageResponse,
    WebChatMessagesResponse, serve_webchat_with_shutdown,
};

use super::support::{wait_for_sent_messages, webchat_fixture};

#[tokio::test]
async fn webchat_server_accepts_http_posts() {
    let (webchat, mut event_rx) = webchat_fixture();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener should have address");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(serve_webchat_with_shutdown(listener, webchat, async move {
        let _ = shutdown_rx.await;
    }));

    let response = reqwest::Client::new()
        .post(format!("http://{address}/api/webchat/conversation-1"))
        .json(&SubmitTextRequest {
            sender_id: "user-1".to_string(),
            text: "hello over http".to_string(),
            image_urls: Vec::new(),
            message_parts: Vec::new(),
        })
        .send()
        .await
        .expect("HTTP request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: SubmitTextResponse = response.json().await.expect("response JSON should parse");
    assert!(!payload.event_id.is_empty());

    let event = event_rx.recv().await.expect("event should be submitted");
    assert_eq!(event.id, payload.event_id);
    assert_eq!(event.session.conversation_id, "conversation-1");
    assert_eq!(event.message.plain_text(), "hello over http");

    let _ = shutdown_tx.send(());
    server
        .await
        .expect("server task should join")
        .expect("server should shut down cleanly");
}

#[tokio::test]
async fn webchat_server_exposes_runtime_replies_as_message_history() {
    let config = RuntimeConfig {
        platforms: vec![RuntimePlatformConfig::webchat("webchat")],
        ..RuntimeConfig::default()
    };
    let runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");
    let webchat = runtime
        .platform_manager()
        .webchat_platform("webchat")
        .expect("webchat platform should exist");
    let runtime_handle = runtime.start();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener should have address");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(serve_webchat_with_shutdown(listener, webchat, async move {
        let _ = shutdown_rx.await;
    }));

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{address}/api/webchat/conversation-1"))
        .json(&SubmitTextRequest {
            sender_id: "user-1".to_string(),
            text: "hello over runtime".to_string(),
            image_urls: Vec::new(),
            message_parts: Vec::new(),
        })
        .send()
        .await
        .expect("HTTP request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let sent = wait_for_sent_messages(&runtime_handle, "webchat", 1).await;
    assert_eq!(sent.len(), 1);

    let history: WebChatMessagesResponse = client
        .get(format!(
            "http://{address}/api/webchat/conversation-1/messages"
        ))
        .send()
        .await
        .expect("history request should succeed")
        .json()
        .await
        .expect("history response should parse");

    assert_eq!(history.conversation_id, "conversation-1");
    assert_eq!(
        history.messages,
        vec![WebChatMessageResponse {
            text: "hello from astrbot-rs".to_string(),
            image_urls: Vec::new(),
            message_parts: vec![WebChatMessagePart::Plain {
                text: "hello from astrbot-rs".to_string(),
            }],
        }]
    );

    let _ = shutdown_tx.send(());
    server
        .await
        .expect("server task should join")
        .expect("server should shut down cleanly");
    runtime_handle
        .stop()
        .await
        .expect("runtime should stop cleanly");
}
