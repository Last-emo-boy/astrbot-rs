use std::sync::Arc;

use astrbot_core::ProviderContextMessage;
use astrbot_provider::{
    ANTHROPIC_CHAT_PROVIDER_TYPE, ChatProvider, ChatProviderConfig, ChatRequest, ProviderManager,
    ProviderRegistry,
};
use tokio::sync::Mutex;

mod support;
use support::captured_request::has_header;
use support::http_server::serve_once;

#[tokio::test]
async fn sends_anthropic_message_request_and_parses_text_response() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"text","text":"hello from claude"}],"model":"claude-test","stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":2}}"#,
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![
            ChatProviderConfig::anthropic("anthropic", base_url, "claude-test")
                .with_api_key("test-key"),
        ],
        Some("anthropic".to_string()),
    )
    .expect("Anthropic provider should build");

    let response = manager
        .chat(
            ChatRequest::new("hello", "session-1")
                .with_system_prompt("be concise")
                .with_context(ProviderContextMessage::text("assistant", "previous answer")),
        )
        .await
        .expect("Anthropic provider should parse response");

    assert_eq!(response.chain.plain_text(), "hello from claude");

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /v1/messages HTTP/1.1"));
    assert!(has_header(&request, "x-api-key", "test-key"));
    assert!(has_header(&request, "anthropic-version", "2023-06-01"));
    assert!(request.contains(r#""model":"claude-test""#));
    assert!(request.contains(r#""max_tokens":1024"#));
    assert!(request.contains(r#""system":"be concise""#));
    assert!(request.contains(r#""role":"assistant""#));
    assert!(request.contains(r#""content":"previous answer""#));
    assert!(request.contains(r#""role":"user""#));
    assert!(request.contains(r#""content":"hello""#));
}

#[tokio::test]
async fn anthropic_provider_accepts_data_url_images() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"content":[{"type":"text","text":"image ok"}]}"#,
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![ChatProviderConfig::anthropic(
            "anthropic",
            base_url,
            "claude-vision",
        )],
        Some("anthropic".to_string()),
    )
    .expect("Anthropic provider should build");

    let response = manager
        .chat(
            ChatRequest::new("", "session-1").with_image_url("data:image/png;base64,iVBORw0KGgo="),
        )
        .await
        .expect("Anthropic image request should parse response");

    assert_eq!(response.chain.plain_text(), "image ok");
    let request = captured.lock().await.clone();
    assert!(request.contains(r#""type":"image""#));
    assert!(request.contains(r#""media_type":"image/png""#));
    assert!(request.contains(r#""data":"iVBORw0KGgo=""#));
    assert!(request.contains(r#""text":"[image]""#));
}

#[tokio::test]
async fn maps_anthropic_error_response_to_provider_error() {
    let base_url = serve_once(
        "400 Bad Request",
        "application/json",
        r#"{"error":{"type":"invalid_request_error","message":"bad anthropic request"}}"#,
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![ChatProviderConfig::anthropic(
            "anthropic",
            base_url,
            "claude-test",
        )],
        Some("anthropic".to_string()),
    )
    .expect("Anthropic provider should build");

    let error = manager
        .chat(ChatRequest::new("hello", "session-1"))
        .await
        .expect_err("Anthropic error should be mapped");

    let message = error.to_string();
    assert!(message.contains("400 Bad Request"));
    assert!(message.contains("bad anthropic request"));
}

#[test]
fn provider_type_matches_astrbot_anthropic_name() {
    assert_eq!(ANTHROPIC_CHAT_PROVIDER_TYPE, "anthropic_chat_completion");
}
