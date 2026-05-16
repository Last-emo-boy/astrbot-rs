use super::*;

#[test]
fn builtins_register_rerank_provider_types() {
    let registry = ProviderRegistry::with_builtin_providers();

    assert!(registry.has_rerank_provider(MOCK_RERANK_PROVIDER_TYPE));
    assert!(registry.has_rerank_provider(VLLM_RERANK_PROVIDER_TYPE));
    assert!(registry.has_rerank_provider(BAILIAN_RERANK_PROVIDER_TYPE));
    assert!(registry.has_rerank_provider(XINFERENCE_RERANK_PROVIDER_TYPE));
    assert_eq!(
        registry
            .provider_metadata(MOCK_RERANK_PROVIDER_TYPE)
            .expect("mock rerank metadata should exist")
            .capability,
        ProviderCapability::Rerank
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::Rerank)
            .contains(&MOCK_RERANK_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::Rerank)
            .contains(&VLLM_RERANK_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::Rerank)
            .contains(&BAILIAN_RERANK_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::Rerank)
            .contains(&XINFERENCE_RERANK_PROVIDER_TYPE.to_string())
    );
}

#[tokio::test]
async fn manager_builds_enabled_rerank_providers_and_selects_default() {
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_rerank_configs(
        &registry,
        vec![
            RerankProviderConfig::mock("disabled", vec![1.0]).disabled(),
            RerankProviderConfig::mock("primary", vec![0.1, 0.9, 0.5]),
            RerankProviderConfig::mock("secondary", vec![0.8, 0.2]),
        ],
        Some("secondary".to_string()),
    )
    .expect("rerank manager should build");

    assert_eq!(manager.rerank_provider_count(), 2);
    assert_eq!(manager.default_rerank_provider_id(), Some("secondary"));

    let response = manager
        .rerank(RerankRequest::new("query", ["doc-a", "doc-b"]))
        .await
        .expect("default rerank provider should respond");

    assert_eq!(
        response.results,
        vec![
            RerankDocumentScore::new(0, 0.8),
            RerankDocumentScore::new(1, 0.2)
        ]
    );
}

#[tokio::test]
async fn manager_routes_rerank_request_to_requested_provider() {
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_rerank_configs(
        &registry,
        vec![
            RerankProviderConfig::mock("primary", vec![0.1, 0.7, 0.3]),
            RerankProviderConfig::mock("secondary", vec![0.4, 0.9, 0.2]),
        ],
        Some("primary".to_string()),
    )
    .expect("rerank manager should build");

    let selected = manager
        .rerank(
            RerankRequest::new("query", ["a", "b", "c"])
                .with_provider_id("secondary")
                .with_top_n(2),
        )
        .await
        .expect("requested rerank provider should respond");
    assert_eq!(
        selected.results,
        vec![
            RerankDocumentScore::new(1, 0.9),
            RerankDocumentScore::new(0, 0.4)
        ]
    );

    let fallback = manager
        .rerank(RerankRequest::new("query", ["a", "b", "c"]).with_top_n(1))
        .await
        .expect("default rerank provider should respond");
    assert_eq!(fallback.results, vec![RerankDocumentScore::new(1, 0.7)]);

    let missing = manager
        .rerank(RerankRequest::new("query", ["a"]).with_provider_id("missing"))
        .await
        .expect_err("missing requested rerank provider should fail");
    assert!(missing.to_string().contains("missing"));
}

#[tokio::test]
async fn manager_builds_vllm_rerank_provider_from_registry() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"results":[{"index":0,"relevance_score":0.8}]}"#,
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_rerank_configs(
        &registry,
        vec![
            RerankProviderConfig::vllm("vllm-rerank", base_url, "BAAI/bge-reranker-base")
                .with_api_key("test-key"),
        ],
        Some("vllm-rerank".to_string()),
    )
    .expect("rerank manager should build");

    let response = manager
        .rerank(RerankRequest::new("query", ["document"]))
        .await
        .expect("VLLM rerank provider should respond");

    assert_eq!(response.results, vec![RerankDocumentScore::new(0, 0.8)]);

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /v1/rerank HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains(r#""model":"BAAI/bge-reranker-base""#));
    assert!(request.contains(r#""query":"query""#));
    assert!(request.contains(r#""documents":["document"]"#));
}

#[tokio::test]
async fn manager_builds_bailian_rerank_provider_from_registry() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"output":{"results":[{"index":0,"relevance_score":0.7}]}}"#,
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_rerank_configs(
        &registry,
        vec![
            RerankProviderConfig::bailian(
                "bailian-rerank",
                format!("{base_url}/rerank"),
                "qwen3-rerank",
            )
            .with_api_key("test-key"),
        ],
        Some("bailian-rerank".to_string()),
    )
    .expect("rerank manager should build");

    let response = manager
        .rerank(RerankRequest::new("query", ["document"]).with_top_n(1))
        .await
        .expect("Bailian rerank provider should respond");

    assert_eq!(response.results, vec![RerankDocumentScore::new(0, 0.7)]);

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /rerank HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains(r#""model":"qwen3-rerank""#));
    assert!(request.contains(r#""query":"query""#));
    assert!(request.contains(r#""documents":["document"]"#));
    assert!(request.contains(r#""top_n":1"#));
}

#[test]
fn manager_builds_xinference_rerank_provider_from_registry() {
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_rerank_configs(
        &registry,
        vec![
            RerankProviderConfig::xinference(
                "xinference-rerank",
                "http://127.0.0.1:1",
                "BAAI/bge-reranker-base",
            )
            .with_api_key("test-key")
            .with_launch_model_if_not_running(true),
        ],
        Some("xinference-rerank".to_string()),
    )
    .expect("Xinference rerank manager should build");

    assert_eq!(manager.rerank_provider_count(), 1);
    assert_eq!(
        manager.default_rerank_provider_id(),
        Some("xinference-rerank")
    );
}
