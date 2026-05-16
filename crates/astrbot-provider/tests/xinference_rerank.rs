use std::sync::Arc;

use astrbot_provider::{
    RerankProvider, RerankRequest, XINFERENCE_RERANK_PROVIDER_TYPE, XinferenceRerankConfig,
    XinferenceRerankProvider,
};
use tokio::sync::Mutex;

mod support;
use support::http_server::{TestResponse, serve_sequence};

#[tokio::test]
async fn resolves_running_model_uid_and_sends_rerank_request() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![
            TestResponse::json(
                "200 OK",
                r#"{"running-reranker":{"model_name":"BAAI/bge-reranker-base","model_type":"rerank"}}"#,
            ),
            TestResponse::json(
                "200 OK",
                r#"{"results":[{"index":1,"relevance_score":0.93},{"index":0,"relevance_score":0.31}]}"#,
            ),
        ],
        captured.clone(),
    )
    .await;
    let provider = XinferenceRerankProvider::new(
        XinferenceRerankConfig::new(base_url, "BAAI/bge-reranker-base").with_api_key("test-key"),
    )
    .expect("provider should build");

    let response = provider
        .rerank(RerankRequest::new("Apple", ["apple document", "banana document"]).with_top_n(2))
        .await
        .expect("provider should parse rerank response");

    assert_eq!(response.results.len(), 2);
    assert_eq!(response.results[0].index, 1);
    assert_eq!(response.results[0].relevance_score, 0.93);
    assert_eq!(response.results[1].index, 0);
    assert_eq!(response.results[1].relevance_score, 0.31);

    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /v1/models HTTP/1.1"));
    assert!(requests[0].contains("authorization: Bearer test-key"));
    assert!(requests[1].starts_with("POST /v1/rerank HTTP/1.1"));
    assert!(requests[1].contains("authorization: Bearer test-key"));
    assert!(requests[1].contains(r#""model":"running-reranker""#));
    assert!(requests[1].contains(r#""query":"Apple""#));
    assert!(requests[1].contains(r#""documents":["apple document","banana document"]"#));
    assert!(requests[1].contains(r#""top_n":2"#));
}

#[tokio::test]
async fn launches_model_when_not_running_and_auto_launch_is_enabled() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![
            TestResponse::json("200 OK", r#"{"data":[]}"#),
            TestResponse::json("200 OK", r#"{"model_uid":"launched-reranker"}"#),
            TestResponse::json(
                "200 OK",
                r#"{"results":[{"index":0,"relevance_score":0.75}]}"#,
            ),
        ],
        captured.clone(),
    )
    .await;
    let provider = XinferenceRerankProvider::new(
        XinferenceRerankConfig::new(base_url, "BAAI/bge-reranker-base")
            .with_launch_model_if_not_running(true),
    )
    .expect("provider should build");

    let response = provider
        .rerank(RerankRequest::new("query", ["document"]))
        .await
        .expect("provider should launch then rerank");

    assert_eq!(response.results[0].index, 0);
    assert_eq!(response.results[0].relevance_score, 0.75);

    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].starts_with("GET /v1/models HTTP/1.1"));
    assert!(requests[1].starts_with("POST /v1/models HTTP/1.1"));
    assert!(requests[1].contains(r#""model_name":"BAAI/bge-reranker-base""#));
    assert!(requests[1].contains(r#""model_type":"rerank""#));
    assert!(requests[2].starts_with("POST /v1/rerank HTTP/1.1"));
    assert!(requests[2].contains(r#""model":"launched-reranker""#));
}

#[tokio::test]
async fn returns_error_when_model_is_not_running_and_auto_launch_is_disabled() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::json("200 OK", r#"{"data":[]}"#)],
        captured.clone(),
    )
    .await;
    let provider = XinferenceRerankProvider::new(XinferenceRerankConfig::new(
        base_url,
        "BAAI/bge-reranker-base",
    ))
    .expect("provider should build");

    let error = provider
        .rerank(RerankRequest::new("query", ["document"]))
        .await
        .expect_err("provider should fail when model is unavailable");

    assert!(error.to_string().contains("auto-launch is disabled"));
    assert_eq!(captured.lock().await.len(), 1);
}

#[test]
fn provider_type_matches_astrbot_xinference_rerank_name() {
    assert_eq!(XINFERENCE_RERANK_PROVIDER_TYPE, "xinference_rerank");
}
