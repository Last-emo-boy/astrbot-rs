use std::sync::Arc;

use astrbot_provider::{
    BAILIAN_RERANK_PROVIDER_TYPE, BailianRerankConfig, BailianRerankProvider, RerankProvider,
    RerankRequest,
};
use tokio::sync::Mutex;

mod support;
use support::http_server::serve_once;

#[tokio::test]
async fn sends_bailian_rerank_request_and_parses_results() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"output":{"results":[{"index":1,"relevance_score":0.92},{"index":0,"relevance_score":null},{"relevance_score":0.25}]},"usage":{"total_tokens":12}}"#,
        captured.clone(),
    )
    .await;
    let provider = BailianRerankProvider::new(
        BailianRerankConfig::new(format!("{base_url}/rerank"), "qwen3-rerank")
            .with_api_key("test-key")
            .with_return_documents(true)
            .with_instruct("Rank by semantic relevance."),
    )
    .expect("provider should build");

    let response = provider
        .rerank(RerankRequest::new("Apple", ["apple document", "banana document"]).with_top_n(2))
        .await
        .expect("provider should parse rerank response");

    assert_eq!(response.results.len(), 3);
    assert_eq!(response.results[0].index, 1);
    assert_eq!(response.results[0].relevance_score, 0.92);
    assert_eq!(response.results[1].index, 0);
    assert_eq!(response.results[1].relevance_score, 0.0);
    assert_eq!(response.results[2].index, 2);
    assert_eq!(response.results[2].relevance_score, 0.25);

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /rerank HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains(r#""model":"qwen3-rerank""#));
    assert!(request.contains(r#""query":"Apple""#));
    assert!(request.contains(r#""documents":["apple document","banana document"]"#));
    assert!(request.contains(r#""top_n":2"#));
    assert!(request.contains(r#""return_documents":true"#));
    assert!(request.contains(r#""instruct":"Rank by semantic relevance.""#));
}

#[tokio::test]
async fn omits_bailian_instruct_for_non_qwen3_model() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"output":{"results":[{"index":0,"relevance_score":0.5}]}}"#,
        captured.clone(),
    )
    .await;
    let provider = BailianRerankProvider::new(
        BailianRerankConfig::new(format!("{base_url}/rerank"), "other-rerank")
            .with_api_key("test-key")
            .with_instruct("Rank by semantic relevance."),
    )
    .expect("provider should build");

    provider
        .rerank(RerankRequest::new("query", ["document"]))
        .await
        .expect("provider should parse rerank response");

    let request = captured.lock().await.clone();
    assert!(request.contains(r#""model":"other-rerank""#));
    assert!(!request.contains("instruct"));
}

#[tokio::test]
async fn maps_bailian_api_error_response_to_provider_error() {
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"code":"InvalidApiKey","message":"invalid key"}"#,
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider = BailianRerankProvider::new(
        BailianRerankConfig::new(format!("{base_url}/rerank"), "qwen3-rerank")
            .with_api_key("bad-key"),
    )
    .expect("provider should build");

    let error = provider
        .rerank(RerankRequest::new("query", ["document"]))
        .await
        .expect_err("provider should map API code error");

    let message = error.to_string();
    assert!(message.contains("InvalidApiKey"));
    assert!(message.contains("invalid key"));
}

#[tokio::test]
async fn maps_bailian_http_error_response_to_provider_error() {
    let base_url = serve_once(
        "401 Unauthorized",
        "application/json",
        r#"{"message":"missing API key"}"#,
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider = BailianRerankProvider::new(
        BailianRerankConfig::new(format!("{base_url}/rerank"), "qwen3-rerank")
            .with_api_key("bad-key"),
    )
    .expect("provider should build");

    let error = provider
        .rerank(RerankRequest::new("query", ["document"]))
        .await
        .expect_err("provider should map non-success status");

    let message = error.to_string();
    assert!(message.contains("401 Unauthorized"));
    assert!(message.contains("missing API key"));
}

#[test]
fn rejects_missing_bailian_api_key() {
    let error = BailianRerankProvider::new(BailianRerankConfig::new(
        "https://dashscope.aliyuncs.com/api/v1/services/rerank/text-rerank/text-rerank",
        "qwen3-rerank",
    ))
    .expect_err("Bailian provider should require API key");

    assert!(error.to_string().contains("API key is required"));
}

#[test]
fn provider_type_matches_astrbot_bailian_rerank_name() {
    assert_eq!(BAILIAN_RERANK_PROVIDER_TYPE, "bailian_rerank");
}
