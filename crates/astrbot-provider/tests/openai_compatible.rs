use std::sync::Arc;

use astrbot_core::{ProviderContentPart, ProviderContextMessage};
use astrbot_provider::{
    ChatProvider, ChatRequest, OpenAiCompatibleConfig, OpenAiCompatibleProvider,
};
use tokio::sync::Mutex;

mod support;
use support::http_server::serve_once;

#[tokio::test]
async fn sends_openai_chat_completion_request_and_parses_text_response() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"choices":[{"message":{"role":"assistant","content":"hello from api"}}]}"#,
        captured.clone(),
    )
    .await;
    let provider = OpenAiCompatibleProvider::new(
        OpenAiCompatibleConfig::new(base_url, "test-model").with_api_key("test-key"),
    )
    .expect("provider should build");

    let response = provider
        .chat(ChatRequest::new("hello", "session-1"))
        .await
        .expect("provider should parse response");

    assert_eq!(response.chain.plain_text(), "hello from api");

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /chat/completions HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains(r#""model":"test-model""#));
    assert!(request.contains(r#""role":"user""#));
    assert!(request.contains(r#""content":"hello""#));
    assert!(request.contains(r#""stream":false"#));
}

#[tokio::test]
async fn sends_openai_multimodal_content_parts_when_images_are_attached() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"choices":[{"message":{"role":"assistant","content":"image received"}}]}"#,
        captured.clone(),
    )
    .await;
    let provider = OpenAiCompatibleProvider::new(
        OpenAiCompatibleConfig::new(base_url, "vision-model").with_api_key("test-key"),
    )
    .expect("provider should build");

    let response = provider
        .chat(
            ChatRequest::new("describe this", "session-1")
                .with_image_url("https://example.test/image.png"),
        )
        .await
        .expect("provider should parse response");

    assert_eq!(response.chain.plain_text(), "image received");

    let request = captured.lock().await.clone();
    assert!(request.contains(r#""model":"vision-model""#));
    assert!(request.contains(r#""content":["#));
    assert!(request.contains(r#""type":"text""#));
    assert!(request.contains(r#""text":"describe this""#));
    assert!(request.contains(r#""type":"image_url""#));
    assert!(request.contains(r#""url":"https://example.test/image.png""#));
}

#[tokio::test]
async fn parses_content_parts_response() {
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"choices":[{"message":{"role":"assistant","content":[{"type":"text","text":"hello"},{"type":"text","text":" parts"}]}}]}"#,
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider =
        OpenAiCompatibleProvider::new(OpenAiCompatibleConfig::new(base_url, "test-model"))
            .expect("provider should build");

    let response = provider
        .chat(ChatRequest::new("hello", "session-1"))
        .await
        .expect("provider should parse response");

    assert_eq!(response.chain.plain_text(), "hello parts");
}

#[tokio::test]
async fn sends_request_model_override_system_prompt_and_contexts() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"choices":[{"message":{"role":"assistant","content":"context ok"}}]}"#,
        captured.clone(),
    )
    .await;
    let provider =
        OpenAiCompatibleProvider::new(OpenAiCompatibleConfig::new(base_url, "default-model"))
            .expect("provider should build");

    let response = provider
        .chat(
            ChatRequest::new("hello", "session-1")
                .with_model("override-model")
                .with_system_prompt("be concise")
                .with_context(ProviderContextMessage::text("assistant", "previous answer"))
                .with_extra_user_content_part(ProviderContentPart::text("extra instruction")),
        )
        .await
        .expect("provider should parse response");

    assert_eq!(response.chain.plain_text(), "context ok");

    let request = captured.lock().await.clone();
    assert!(request.contains(r#""model":"override-model""#));
    assert!(request.contains(r#""role":"system""#));
    assert!(request.contains(r#""content":"be concise""#));
    assert!(request.contains(r#""role":"assistant""#));
    assert!(request.contains(r#""content":"previous answer""#));
    assert!(request.contains(r#""content":["#));
    assert!(request.contains(r#""text":"hello""#));
    assert!(request.contains(r#""text":"extra instruction""#));
}

#[tokio::test]
async fn maps_error_response_to_provider_error() {
    let base_url = serve_once(
        "400 Bad Request",
        "application/json",
        r#"{"error":{"message":"invalid model"}}"#,
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider =
        OpenAiCompatibleProvider::new(OpenAiCompatibleConfig::new(base_url, "bad-model"))
            .expect("provider should build");

    let error = provider
        .chat(ChatRequest::new("hello", "session-1"))
        .await
        .expect_err("provider should map non-success status");

    let message = error.to_string();
    assert!(message.contains("400 Bad Request"));
    assert!(message.contains("invalid model"));
}

#[tokio::test]
async fn streams_openai_chat_completion_request_and_collects_chunks() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "text/event-stream",
        concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
            "data: [DONE]\n\n"
        ),
        captured.clone(),
    )
    .await;
    let provider = OpenAiCompatibleProvider::new(
        OpenAiCompatibleConfig::new(base_url, "test-model").with_api_key("test-key"),
    )
    .expect("provider should build");

    let response = provider
        .chat(ChatRequest::new("hello", "session-1").with_stream(true))
        .await
        .expect("provider should parse streaming response");

    assert_eq!(response.chain.plain_text(), "hello world");

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /chat/completions HTTP/1.1"));
    assert!(request.contains(r#""stream":true"#));
}
