use std::sync::Arc;

use astrbot_provider::{
    EmbeddingProvider, EmbeddingRequest, OPENAI_EMBEDDING_PROVIDER_TYPE, OpenAiEmbeddingConfig,
    OpenAiEmbeddingProvider,
};
use tokio::sync::Mutex;

mod support;
use support::http_server::serve_once;

#[tokio::test]
async fn sends_openai_embedding_request_and_parses_vectors() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"data":[{"index":0,"embedding":[0.1,0.2]},{"index":1,"embedding":[0.3,0.4]}]}"#,
        captured.clone(),
    )
    .await;
    let provider = OpenAiEmbeddingProvider::new(
        OpenAiEmbeddingConfig::new(base_url, "text-embedding-3-small")
            .with_api_key("test-key")
            .with_dimensions(2),
    )
    .expect("provider should build");

    let response = provider
        .embed(EmbeddingRequest::batch(["first", "second"]).with_model("override-embedding"))
        .await
        .expect("provider should parse embedding response");

    assert_eq!(response.embeddings, vec![vec![0.1, 0.2], vec![0.3, 0.4]]);
    assert_eq!(response.dimension(), Some(2));
    assert_eq!(provider.dimensions(), Some(2));

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /embeddings HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains(r#""model":"override-embedding""#));
    assert!(request.contains(r#""input":["first","second"]"#));
    assert!(request.contains(r#""dimensions":2"#));
}

#[tokio::test]
async fn sends_single_text_embedding_input_as_string() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"data":[{"index":0,"embedding":[1.0,2.0]}]}"#,
        captured.clone(),
    )
    .await;
    let provider = OpenAiEmbeddingProvider::new(OpenAiEmbeddingConfig::new(
        base_url,
        "text-embedding-3-small",
    ))
    .expect("provider should build");

    let response = provider
        .embed(EmbeddingRequest::new("hello"))
        .await
        .expect("provider should parse embedding response");

    assert_eq!(response.embeddings, vec![vec![1.0, 2.0]]);

    let request = captured.lock().await.clone();
    assert!(request.contains(r#""input":"hello""#));
    assert!(request.contains(r#""dimensions":1024"#));
}

#[tokio::test]
async fn maps_openai_embedding_error_response_to_provider_error() {
    let base_url = serve_once(
        "400 Bad Request",
        "application/json",
        r#"{"error":{"message":"invalid embedding model"}}"#,
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider = OpenAiEmbeddingProvider::new(OpenAiEmbeddingConfig::new(base_url, "bad-model"))
        .expect("provider should build");

    let error = provider
        .embed(EmbeddingRequest::new("hello"))
        .await
        .expect_err("provider should map non-success status");

    let message = error.to_string();
    assert!(message.contains("400 Bad Request"));
    assert!(message.contains("invalid embedding model"));
}

#[test]
fn provider_type_matches_astrbot_openai_embedding_name() {
    assert_eq!(OPENAI_EMBEDDING_PROVIDER_TYPE, "openai_embedding");
}
