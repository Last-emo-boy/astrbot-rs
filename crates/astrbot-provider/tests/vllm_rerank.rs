use std::sync::Arc;

use astrbot_provider::{
    RerankProvider, RerankRequest, VLLM_RERANK_PROVIDER_TYPE, VllmRerankConfig, VllmRerankProvider,
};
use tokio::sync::Mutex;

mod support;
use support::http_server::serve_once;

#[tokio::test]
async fn sends_vllm_rerank_request_and_parses_results() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"results":[{"index":1,"relevance_score":0.9},{"index":0,"relevance_score":0.4}]}"#,
        captured.clone(),
    )
    .await;
    let provider = VllmRerankProvider::new(
        VllmRerankConfig::new(base_url, "BAAI/bge-reranker-base").with_api_key("test-key"),
    )
    .expect("provider should build");

    let response = provider
        .rerank(
            RerankRequest::new("Apple", ["apple document", "banana document"])
                .with_top_n(2)
                .with_model("override-reranker"),
        )
        .await
        .expect("provider should parse rerank response");

    assert_eq!(response.results.len(), 2);
    assert_eq!(response.results[0].index, 1);
    assert_eq!(response.results[0].relevance_score, 0.9);
    assert_eq!(response.results[1].index, 0);
    assert_eq!(response.results[1].relevance_score, 0.4);

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /v1/rerank HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains(r#""query":"Apple""#));
    assert!(request.contains(r#""documents":["apple document","banana document"]"#));
    assert!(request.contains(r#""model":"override-reranker""#));
    assert!(request.contains(r#""top_n":2"#));
}

#[tokio::test]
async fn maps_vllm_rerank_error_response_to_provider_error() {
    let base_url = serve_once(
        "400 Bad Request",
        "application/json",
        r#"{"error":{"message":"invalid rerank model"}}"#,
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider = VllmRerankProvider::new(VllmRerankConfig::new(base_url, "bad-model"))
        .expect("provider should build");

    let error = provider
        .rerank(RerankRequest::new("query", ["document"]))
        .await
        .expect_err("provider should map non-success status");

    let message = error.to_string();
    assert!(message.contains("400 Bad Request"));
    assert!(message.contains("invalid rerank model"));
}

#[test]
fn provider_type_matches_astrbot_vllm_rerank_name() {
    assert_eq!(VLLM_RERANK_PROVIDER_TYPE, "vllm_rerank");
}
