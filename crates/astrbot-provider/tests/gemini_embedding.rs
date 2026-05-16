use std::sync::Arc;

use astrbot_provider::{
    EmbeddingProvider, EmbeddingRequest, GEMINI_EMBEDDING_PROVIDER_TYPE, GeminiEmbeddingConfig,
    GeminiEmbeddingProvider,
};
use tokio::sync::Mutex;

mod support;
use support::http_server::serve_once;

#[tokio::test]
async fn sends_gemini_single_embedding_request_and_parses_vector() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"embedding":{"values":[0.1,0.2,0.3]}}"#,
        captured.clone(),
    )
    .await;
    let provider = GeminiEmbeddingProvider::new(
        GeminiEmbeddingConfig::new(base_url, "gemini-embedding-001")
            .with_api_key("test-key")
            .with_dimensions(3),
    )
    .expect("provider should build");

    let response = provider
        .embed(EmbeddingRequest::new("hello"))
        .await
        .expect("provider should parse embedding response");

    assert_eq!(response.embeddings, vec![vec![0.1, 0.2, 0.3]]);
    assert_eq!(response.dimension(), Some(3));
    assert_eq!(provider.dimensions(), Some(3));

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /v1beta/models/gemini-embedding-001:embedContent HTTP/1.1"));
    assert!(request.contains("x-goog-api-key: test-key"));
    assert!(request.contains(r#""model":"models/gemini-embedding-001""#));
    assert!(request.contains(r#""text":"hello""#));
    assert!(request.contains(r#""outputDimensionality":3"#));
}

#[tokio::test]
async fn sends_gemini_batch_embedding_request_and_parses_vectors() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"embeddings":[{"values":[1.0,2.0]},{"values":[3.0,4.0]}]}"#,
        captured.clone(),
    )
    .await;
    let provider =
        GeminiEmbeddingProvider::new(GeminiEmbeddingConfig::new(base_url, "models/custom-embed"))
            .expect("provider should build");

    let response = provider
        .embed(EmbeddingRequest::batch(["first", "second"]).with_model("gemini-embedding-001"))
        .await
        .expect("provider should parse batch embedding response");

    assert_eq!(response.embeddings, vec![vec![1.0, 2.0], vec![3.0, 4.0]]);

    let request = captured.lock().await.clone();
    assert!(
        request.starts_with("POST /v1beta/models/gemini-embedding-001:batchEmbedContents HTTP/1.1")
    );
    assert!(request.contains(r#""requests":["#));
    assert!(request.contains(r#""model":"models/gemini-embedding-001""#));
    assert!(request.contains(r#""text":"first""#));
    assert!(request.contains(r#""text":"second""#));
    assert!(request.contains(r#""outputDimensionality":768"#));
}

#[tokio::test]
async fn maps_gemini_embedding_error_response_to_provider_error() {
    let base_url = serve_once(
        "400 Bad Request",
        "application/json",
        r#"{"error":{"message":"invalid embedding model"}}"#,
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider = GeminiEmbeddingProvider::new(GeminiEmbeddingConfig::new(base_url, "bad-model"))
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
fn provider_type_matches_astrbot_gemini_embedding_name() {
    assert_eq!(GEMINI_EMBEDDING_PROVIDER_TYPE, "gemini_embedding");
}
