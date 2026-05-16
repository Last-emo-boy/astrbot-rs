use std::sync::Arc;

use astrbot_core::{ProviderContentPart, ProviderContextMessage};
use astrbot_provider::{
    ChatProvider, ChatProviderConfig, ChatRequest, GOOGLE_GENAI_CHAT_PROVIDER_TYPE,
    ProviderManager, ProviderRegistry,
};
use tokio::sync::Mutex;

mod support;
use support::captured_request::has_header;
use support::http_server::serve_once;

#[tokio::test]
async fn sends_gemini_generate_content_request_and_parses_text_response() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"candidates":[{"content":{"parts":[{"text":"hello from gemini"}],"role":"model"},"finishReason":"STOP"}]}"#,
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![
            ChatProviderConfig::google_genai("gemini", base_url, "gemini-test")
                .with_api_key("test-key"),
        ],
        Some("gemini".to_string()),
    )
    .expect("Gemini provider should build");

    let response = manager
        .chat(
            ChatRequest::new("hello", "session-1")
                .with_system_prompt("be concise")
                .with_context(ProviderContextMessage::text("assistant", "previous answer")),
        )
        .await
        .expect("Gemini provider should parse response");

    assert_eq!(response.chain.plain_text(), "hello from gemini");

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /v1beta/models/gemini-test:generateContent HTTP/1.1"));
    assert!(has_header(&request, "x-goog-api-key", "test-key"));
    assert!(request.contains(r#""systemInstruction":{"parts":[{"text":"be concise"}]}"#));
    assert!(request.contains(r#""role":"model""#));
    assert!(request.contains(r#""text":"previous answer""#));
    assert!(request.contains(r#""role":"user""#));
    assert!(request.contains(r#""text":"hello""#));
}

#[tokio::test]
async fn gemini_provider_accepts_data_url_images() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"candidates":[{"content":{"parts":[{"text":"image ok"}],"role":"model"},"finishReason":"STOP"}]}"#,
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![ChatProviderConfig::google_genai(
            "gemini",
            base_url,
            "gemini-vision",
        )],
        Some("gemini".to_string()),
    )
    .expect("Gemini provider should build");

    let response = manager
        .chat(
            ChatRequest::new("", "session-1").with_image_url("data:image/png;base64,iVBORw0KGgo="),
        )
        .await
        .expect("Gemini image request should parse response");

    assert_eq!(response.chain.plain_text(), "image ok");
    let request = captured.lock().await.clone();
    assert!(request.contains(r#""text":"[image]""#));
    assert!(request.contains(r#""inlineData":{"mimeType":"image/png","data":"iVBORw0KGgo="}"#));
}

#[tokio::test]
async fn gemini_provider_maps_safety_finish_reason_to_error() {
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"candidates":[{"content":{"parts":[],"role":"model"},"finishReason":"SAFETY"}]}"#,
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![ChatProviderConfig::google_genai(
            "gemini",
            base_url,
            "gemini-test",
        )],
        Some("gemini".to_string()),
    )
    .expect("Gemini provider should build");

    let error = manager
        .chat(ChatRequest::new("hello", "session-1"))
        .await
        .expect_err("Gemini safety finish reason should be mapped");

    let message = error.to_string();
    assert!(message.contains("Gemini provider blocked response"));
    assert!(message.contains("SAFETY"));
}

#[tokio::test]
async fn gemini_provider_maps_policy_finish_reason_to_error() {
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"candidates":[{"content":{"parts":[],"role":"model"},"finishReason":"PROHIBITED_CONTENT"}]}"#,
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![ChatProviderConfig::google_genai(
            "gemini",
            base_url,
            "gemini-test",
        )],
        Some("gemini".to_string()),
    )
    .expect("Gemini provider should build");

    let error = manager
        .chat(ChatRequest::new("hello", "session-1"))
        .await
        .expect_err("Gemini policy finish reason should be mapped");

    assert!(error.to_string().contains("PROHIBITED_CONTENT"));
}

#[tokio::test]
async fn maps_gemini_error_response_to_provider_error() {
    let base_url = serve_once(
        "400 Bad Request",
        "application/json",
        r#"{"error":{"code":400,"message":"bad gemini request","status":"INVALID_ARGUMENT"}}"#,
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![ChatProviderConfig::google_genai(
            "gemini",
            base_url,
            "gemini-test",
        )],
        Some("gemini".to_string()),
    )
    .expect("Gemini provider should build");

    let error = manager
        .chat(ChatRequest::new("hello", "session-1"))
        .await
        .expect_err("Gemini error should be mapped");

    let message = error.to_string();
    assert!(message.contains("400 Bad Request"));
    assert!(message.contains("bad gemini request"));
}

#[test]
fn provider_type_matches_astrbot_gemini_name() {
    assert_eq!(
        GOOGLE_GENAI_CHAT_PROVIDER_TYPE,
        "googlegenai_chat_completion"
    );
}

#[tokio::test]
async fn rejects_remote_image_urls_until_transport_download_exists() {
    let config = ChatProviderConfig::google_genai("gemini", "http://127.0.0.1:1", "gemini-test");
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager =
        ProviderManager::from_chat_configs(&registry, vec![config], Some("gemini".to_string()))
            .expect("Gemini provider should build");

    let error = manager
        .chat(
            ChatRequest::new("hello", "session-1").with_extra_user_content_part(
                ProviderContentPart::image_url("https://example.test/image.png"),
            ),
        )
        .await
        .expect_err("remote image URL should be rejected before HTTP");

    assert!(error.to_string().contains("data URLs"));
}
