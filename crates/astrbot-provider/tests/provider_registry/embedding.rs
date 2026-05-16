use super::*;

#[test]
fn builtins_register_embedding_provider_types() {
    let registry = ProviderRegistry::with_builtin_providers();

    assert!(registry.has_embedding_provider(MOCK_EMBEDDING_PROVIDER_TYPE));
    assert!(registry.has_embedding_provider(OPENAI_EMBEDDING_PROVIDER_TYPE));
    assert!(registry.has_embedding_provider(GEMINI_EMBEDDING_PROVIDER_TYPE));
    assert_eq!(
        registry
            .provider_metadata(MOCK_EMBEDDING_PROVIDER_TYPE)
            .expect("mock embedding metadata should exist")
            .capability,
        ProviderCapability::Embedding
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::Embedding)
            .contains(&MOCK_EMBEDDING_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::Embedding)
            .contains(&OPENAI_EMBEDDING_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::Embedding)
            .contains(&GEMINI_EMBEDDING_PROVIDER_TYPE.to_string())
    );
}

#[tokio::test]
async fn manager_builds_enabled_embedding_providers_and_selects_default() {
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_embedding_configs(
        &registry,
        vec![
            EmbeddingProviderConfig::mock("disabled", vec![0.0]).disabled(),
            EmbeddingProviderConfig::mock("primary", vec![1.0, 2.0, 3.0]),
            EmbeddingProviderConfig::mock("secondary", vec![4.0, 5.0]),
        ],
        Some("secondary".to_string()),
    )
    .expect("embedding manager should build");

    assert_eq!(manager.embedding_provider_count(), 2);
    assert_eq!(manager.default_embedding_provider_id(), Some("secondary"));
    assert_eq!(manager.dimensions(), Some(2));

    let provider = manager
        .default_embedding_provider()
        .expect("default embedding provider should exist");
    let response = provider
        .embed(EmbeddingRequest::new("hello"))
        .await
        .expect("mock embedding provider should respond");

    assert_eq!(response.embeddings, vec![vec![4.0, 5.0]]);
    assert_eq!(response.dimension(), Some(2));
}

#[tokio::test]
async fn manager_routes_embedding_request_to_requested_provider() {
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_embedding_configs(
        &registry,
        vec![
            EmbeddingProviderConfig::mock("primary", vec![1.0]),
            EmbeddingProviderConfig::mock("secondary", vec![2.0, 3.0]),
        ],
        Some("primary".to_string()),
    )
    .expect("embedding manager should build");

    let selected = manager
        .embed(EmbeddingRequest::batch(["a", "b"]).with_provider_id("secondary"))
        .await
        .expect("requested embedding provider should respond");
    assert_eq!(selected.embeddings, vec![vec![2.0, 3.0], vec![2.0, 3.0]]);

    let fallback = manager
        .embed(EmbeddingRequest::new("hello"))
        .await
        .expect("default embedding provider should respond");
    assert_eq!(fallback.embeddings, vec![vec![1.0]]);

    let missing = manager
        .embed(EmbeddingRequest::new("hello").with_provider_id("missing"))
        .await
        .expect_err("missing requested embedding provider should fail");
    assert!(missing.to_string().contains("missing"));
}

#[tokio::test]
async fn manager_builds_openai_embedding_provider_from_registry() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"data":[{"index":0,"embedding":[0.5,0.25]}]}"#,
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_embedding_configs(
        &registry,
        vec![
            EmbeddingProviderConfig::openai("openai-embedding", base_url, "text-embedding-3-small")
                .with_api_key("test-key")
                .with_dimensions(2),
        ],
        Some("openai-embedding".to_string()),
    )
    .expect("embedding manager should build");

    let response = manager
        .embed(EmbeddingRequest::new("hello"))
        .await
        .expect("OpenAI embedding provider should respond");

    assert_eq!(response.embeddings, vec![vec![0.5, 0.25]]);
    assert_eq!(manager.dimensions(), Some(2));

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /v1/embeddings HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains(r#""model":"text-embedding-3-small""#));
    assert!(request.contains(r#""input":"hello""#));
    assert!(request.contains(r#""dimensions":2"#));
}

#[tokio::test]
async fn manager_builds_gemini_embedding_provider_from_registry() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"embedding":{"values":[0.75,0.5]}}"#,
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_embedding_configs(
        &registry,
        vec![
            EmbeddingProviderConfig::gemini("gemini-embedding", base_url, "gemini-embedding-001")
                .with_api_key("test-key")
                .with_dimensions(2),
        ],
        Some("gemini-embedding".to_string()),
    )
    .expect("embedding manager should build");

    let response = manager
        .embed(EmbeddingRequest::new("hello"))
        .await
        .expect("Gemini embedding provider should respond");

    assert_eq!(response.embeddings, vec![vec![0.75, 0.5]]);
    assert_eq!(manager.dimensions(), Some(2));

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /v1beta/models/gemini-embedding-001:embedContent HTTP/1.1"));
    assert!(request.contains("x-goog-api-key: test-key"));
    assert!(request.contains(r#""model":"models/gemini-embedding-001""#));
    assert!(!request.contains(r#""input":"hello""#));
    assert!(request.contains(r#""text":"hello""#));
    assert!(request.contains(r#""outputDimensionality":2"#));
}
