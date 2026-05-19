use astrbot_runtime::{AstrbotRuntime, RuntimeConfig, RuntimePlatformConfig};
use axum::http::StatusCode;
use std::fs;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::{
    ManagementApiState, ManagementStatusResponse, SubmitTextRequest, SubmitTextResponse,
    WebChatMessagePart, WebChatMessageResponse, WebChatMessagesResponse, dashboard_router,
    serve_webchat_with_shutdown,
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

#[tokio::test]
async fn dashboard_router_combines_management_webchat_and_spa_assets() {
    let root = temp_dashboard_root("combined");
    let assets_dir = root.join("dist");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(assets_dir.join("assets")).expect("asset dir should create");
    fs::write(assets_dir.join("index.html"), "<main>AstrBot RS</main>")
        .expect("index should write");
    fs::write(
        assets_dir.join("assets/app.js"),
        "console.log('astrbot-rs')",
    )
    .expect("asset should write");

    let config = RuntimeConfig {
        platforms: vec![RuntimePlatformConfig::webchat("webchat")],
        paths: astrbot_runtime::RuntimePathConfig::default().with_data_dir(&root),
        ..RuntimeConfig::default()
    };
    let runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");
    let webchat = runtime
        .platform_manager()
        .webchat_platform("webchat")
        .expect("webchat platform should exist");
    let management = ManagementApiState::from_managers(
        runtime.provider_manager(),
        runtime.platform_manager(),
        &runtime.plugin_registry(),
    );
    let assets = astrbot_runtime::DashboardAssetPolicy::new(&assets_dir).select();
    let router = dashboard_router(webchat, management, assets);

    let status_response = super::support::get(router.clone(), "/api/management/status").await;
    assert_eq!(status_response.status(), StatusCode::OK);
    let status: ManagementStatusResponse = super::support::response_json(status_response).await;
    assert_eq!(status.platforms.platform_ids, vec!["webchat".to_string()]);

    let submit_response = super::support::post_json(
        router.clone(),
        "/api/webchat/conversation-1",
        serde_json::json!({
            "sender_id": "user-1",
            "text": "hello dashboard"
        }),
    )
    .await;
    assert_eq!(submit_response.status(), StatusCode::OK);

    for route in [
        "/chat",
        "/chat/conversation-1",
        "/chatbox/conversation-1",
        "/knowledge-base/kb-1/document/doc-1",
        "/logs",
        "/tool-use",
        "/alkaid/long-term-memory",
    ] {
        let index_response = super::support::get(router.clone(), route).await;
        assert_eq!(index_response.status(), StatusCode::OK, "{route}");
    }

    let missing_response = super::support::get(router.clone(), "/missing-route").await;
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);

    let asset_response = super::support::get(router, "/assets/app.js").await;
    assert_eq!(asset_response.status(), StatusCode::OK);

    let _ = fs::remove_dir_all(root);
}

fn temp_dashboard_root(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "astrbot-dashboard-router-{}-{suffix}",
        std::process::id()
    ))
}
