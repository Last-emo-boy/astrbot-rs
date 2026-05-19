use std::sync::Arc;
use std::time::Duration;

use astrbot_provider::{
    ANTHROPIC_CHAT_PROVIDER_TYPE, GOOGLE_GENAI_CHAT_PROVIDER_TYPE, OPENAI_CHAT_PROVIDER_TYPE,
    ProviderModelDiscoveryConfig, ProviderModelDiscoverySupport,
    XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE, discover_provider_models,
    sanitize_model_discovery_error,
};
use tokio::sync::Mutex;

mod support;
use support::http_server::{TestResponse, serve_sequence};

#[tokio::test]
async fn discovers_openai_compatible_models() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::json(
            "200 OK",
            r#"{"data":[{"id":"gpt-live"},{"id":"gpt-next"}]}"#,
        )],
        captured.clone(),
    )
    .await;

    let result = discover_provider_models(
        ProviderModelDiscoveryConfig::new(OPENAI_CHAT_PROVIDER_TYPE)
            .with_api_base(base_url)
            .with_api_key("sk-live"),
    )
    .await
    .expect("OpenAI-compatible discovery should succeed");

    assert_eq!(result.support, ProviderModelDiscoverySupport::Supported);
    assert_eq!(
        result
            .models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        vec!["gpt-live", "gpt-next"]
    );
    let request = captured.lock().await.join("\n");
    assert!(request.contains("GET /models HTTP/1.1"));
    assert!(request.contains("authorization: Bearer sk-live"));
}

#[tokio::test]
async fn discovers_gemini_models_and_filters_non_generate_content_models() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::json(
            "200 OK",
            r#"{"models":[{"name":"models/gemini-live","displayName":"Gemini Live","supportedGenerationMethods":["generateContent"]},{"name":"models/embed-only","supportedGenerationMethods":["embedContent"]}]}"#,
        )],
        captured.clone(),
    )
    .await;

    let result = discover_provider_models(
        ProviderModelDiscoveryConfig::new(GOOGLE_GENAI_CHAT_PROVIDER_TYPE)
            .with_api_base(base_url)
            .with_api_key("gm-key"),
    )
    .await
    .expect("Gemini discovery should succeed");

    assert_eq!(result.models.len(), 1);
    assert_eq!(result.models[0].id, "gemini-live");
    assert_eq!(
        result.models[0].display_name.as_deref(),
        Some("Gemini Live")
    );
    let request = captured.lock().await.join("\n");
    assert!(request.contains("GET /v1beta/models HTTP/1.1"));
    assert!(request.contains("x-goog-api-key: gm-key"));
}

#[tokio::test]
async fn discovers_xinference_running_models() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::json(
            "200 OK",
            r#"{"running-whisper":{"model_name":"whisper-large-v3","model_type":"audio"}}"#,
        )],
        captured.clone(),
    )
    .await;

    let result = discover_provider_models(
        ProviderModelDiscoveryConfig::new(XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE)
            .with_api_base(base_url)
            .with_api_key("xinference-key")
            .with_timeout(Duration::from_secs(5)),
    )
    .await
    .expect("Xinference discovery should succeed");

    assert_eq!(result.models.len(), 1);
    assert_eq!(result.models[0].id, "running-whisper");
    assert_eq!(
        result.models[0]
            .metadata
            .get("model_type")
            .map(String::as_str),
        Some("audio")
    );
    let request = captured.lock().await.join("\n");
    assert!(request.contains("GET /v1/models HTTP/1.1"));
    assert!(request.contains("authorization: Bearer xinference-key"));
}

#[tokio::test]
async fn anthropic_model_discovery_is_explicitly_unsupported() {
    let result = discover_provider_models(
        ProviderModelDiscoveryConfig::new(ANTHROPIC_CHAT_PROVIDER_TYPE)
            .with_api_base("http://127.0.0.1:1")
            .with_api_key("anthropic-key"),
    )
    .await
    .expect("unsupported discovery should be represented as a result");

    assert_eq!(result.support, ProviderModelDiscoverySupport::Unsupported);
    assert!(result.unsupported);
    assert_eq!(result.models.len(), 0);
    assert!(
        result
            .message
            .expect("unsupported message")
            .contains("does not expose")
    );
}

#[tokio::test]
async fn discovery_error_can_redact_secret_from_provider_body() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::json(
            "401 Unauthorized",
            r#"{"error":{"message":"token sk-secret was rejected"}}"#,
        )],
        captured,
    )
    .await;

    let error = discover_provider_models(
        ProviderModelDiscoveryConfig::new(OPENAI_CHAT_PROVIDER_TYPE)
            .with_api_base(base_url)
            .with_api_key("sk-secret"),
    )
    .await
    .expect_err("401 should fail discovery");
    let message = sanitize_model_discovery_error(&error, &["sk-secret"]);

    assert!(message.contains("<redacted>"));
    assert!(!message.contains("sk-secret"));
}
