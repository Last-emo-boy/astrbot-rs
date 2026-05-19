use super::*;

#[test]
fn builtins_register_chat_provider_types() {
    let registry = ProviderRegistry::with_builtin_chat_providers();

    assert!(registry.has_chat_provider(MOCK_CHAT_PROVIDER_TYPE));
    assert!(registry.has_chat_provider(ANTHROPIC_CHAT_PROVIDER_TYPE));
    assert!(registry.has_chat_provider(GOOGLE_GENAI_CHAT_PROVIDER_TYPE));
    for provider_type in OPENAI_COMPATIBLE_CHAT_PROVIDER_TYPES {
        assert!(registry.has_chat_provider(provider_type));
    }
}

#[test]
fn builtins_expose_chat_capability_metadata() {
    let registry = ProviderRegistry::with_builtin_chat_providers();

    assert_eq!(
        registry
            .provider_metadata(GOOGLE_GENAI_CHAT_PROVIDER_TYPE)
            .expect("Gemini metadata should exist")
            .capability,
        ProviderCapability::ChatCompletion
    );
    assert_eq!(
        registry
            .provider_metadata(ANTHROPIC_CHAT_PROVIDER_TYPE)
            .expect("Anthropic metadata should exist")
            .capability,
        ProviderCapability::ChatCompletion
    );
    assert_eq!(
        registry
            .provider_metadata(GOOGLE_GENAI_CHAT_PROVIDER_TYPE)
            .expect("Gemini metadata should exist")
            .model_discovery
            .as_str(),
        "supported"
    );
    let anthropic_metadata = registry
        .provider_metadata(ANTHROPIC_CHAT_PROVIDER_TYPE)
        .expect("Anthropic metadata should exist");
    assert_eq!(anthropic_metadata.model_discovery.as_str(), "unsupported");
    assert!(
        anthropic_metadata
            .model_candidates
            .iter()
            .any(|model| model.id == "claude-3-5-sonnet-latest")
    );

    let chat_types = registry.provider_types_by_capability(ProviderCapability::ChatCompletion);
    assert!(chat_types.contains(&MOCK_CHAT_PROVIDER_TYPE.to_string()));
    assert!(chat_types.contains(&GOOGLE_GENAI_CHAT_PROVIDER_TYPE.to_string()));
    for provider_type in OPENAI_COMPATIBLE_CHAT_PROVIDER_TYPES {
        assert!(chat_types.contains(&provider_type.to_string()));
    }
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::Embedding)
            .is_empty()
    );
}

#[test]
fn builtins_register_speech_provider_types() {
    let registry = ProviderRegistry::with_builtin_providers();

    assert!(registry.has_speech_to_text_provider(MOCK_SPEECH_TO_TEXT_PROVIDER_TYPE));
    assert!(registry.has_speech_to_text_provider(OPENAI_SPEECH_TO_TEXT_PROVIDER_TYPE));
    assert!(registry.has_speech_to_text_provider(XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE));
    assert!(
        registry.has_speech_to_text_provider(OPENAI_WHISPER_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE)
    );
    assert!(registry.has_speech_to_text_provider(SENSEVOICE_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE));
    assert!(registry.has_text_to_speech_provider(MOCK_TEXT_TO_SPEECH_PROVIDER_TYPE));
    assert!(registry.has_text_to_speech_provider(OPENAI_TEXT_TO_SPEECH_PROVIDER_TYPE));
    assert!(registry.has_text_to_speech_provider(GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE));
    assert!(registry.has_text_to_speech_provider(VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE));
    assert!(registry.has_text_to_speech_provider(MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE));
    assert!(registry.has_text_to_speech_provider(GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE));
    assert!(registry.has_text_to_speech_provider(GSV_SELFHOST_TEXT_TO_SPEECH_PROVIDER_TYPE));
    assert_eq!(
        registry
            .provider_metadata(MOCK_SPEECH_TO_TEXT_PROVIDER_TYPE)
            .expect("mock speech-to-text metadata should exist")
            .capability,
        ProviderCapability::SpeechToText
    );
    assert_eq!(
        registry
            .provider_metadata(MOCK_TEXT_TO_SPEECH_PROVIDER_TYPE)
            .expect("mock text-to-speech metadata should exist")
            .capability,
        ProviderCapability::TextToSpeech
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::SpeechToText)
            .contains(&MOCK_SPEECH_TO_TEXT_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::SpeechToText)
            .contains(&OPENAI_SPEECH_TO_TEXT_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::SpeechToText)
            .contains(&XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::SpeechToText)
            .contains(&OPENAI_WHISPER_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::SpeechToText)
            .contains(&SENSEVOICE_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::TextToSpeech)
            .contains(&MOCK_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::TextToSpeech)
            .contains(&OPENAI_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::TextToSpeech)
            .contains(&GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::TextToSpeech)
            .contains(&VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::TextToSpeech)
            .contains(&MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::TextToSpeech)
            .contains(&GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string())
    );
    assert!(
        registry
            .provider_types_by_capability(ProviderCapability::TextToSpeech)
            .contains(&GSV_SELFHOST_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string())
    );
}

#[test]
fn duplicate_chat_provider_type_is_rejected() {
    let mut registry = ProviderRegistry::new();
    registry
        .register_chat_provider("custom", |_| {
            Ok(Arc::new(astrbot_provider::MockChatProvider::new("one")))
        })
        .expect("first registration should pass");

    let error = registry
        .register_chat_provider("custom", |_| {
            Ok(Arc::new(astrbot_provider::MockChatProvider::new("two")))
        })
        .expect_err("duplicate registration should fail");

    assert!(error.to_string().contains("already registered"));
}

#[test]
fn non_chat_adapter_metadata_cannot_be_built_as_chat_provider() {
    let mut registry = ProviderRegistry::new();
    registry
        .register_provider_adapter("embedding-only", ProviderCapability::Embedding)
        .expect("metadata registration should pass");

    assert!(registry.has_provider_adapter("embedding-only"));
    assert!(!registry.has_chat_provider("embedding-only"));
    assert_eq!(
        registry
            .provider_metadata("embedding-only")
            .expect("metadata should exist")
            .capability,
        ProviderCapability::Embedding
    );

    let result = ProviderManager::from_chat_configs(
        &registry,
        vec![ChatProviderConfig {
            id: "embedding".to_string(),
            provider_type: "embedding-only".to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout: std::time::Duration::from_secs(1),
            custom_headers: Default::default(),
            mock_response: None,
        }],
        Some("embedding".to_string()),
    );
    let error = match result {
        Ok(_) => panic!("non-chat capability must not build as chat"),
        Err(error) => error,
    };

    let message = error.to_string();
    assert!(message.contains("embedding"));
    assert!(message.contains("chat provider"));
}

#[tokio::test]
async fn manager_builds_enabled_providers_and_selects_default() {
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![
            ChatProviderConfig::mock("disabled", "disabled").disabled(),
            ChatProviderConfig::mock("primary", "primary response"),
            ChatProviderConfig::mock("secondary", "secondary response"),
        ],
        Some("secondary".to_string()),
    )
    .expect("manager should build");

    assert_eq!(manager.chat_provider_count(), 2);
    assert_eq!(manager.default_chat_provider_id(), Some("secondary"));

    let provider = manager
        .default_chat_provider()
        .expect("default provider should exist");
    let response = provider
        .chat(ChatRequest::new("hello", "session"))
        .await
        .expect("mock provider should respond");

    assert_eq!(response.chain.plain_text(), "secondary response");
}

#[test]
fn manager_falls_back_to_first_provider_when_default_is_missing() {
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![
            ChatProviderConfig::mock("first", "first response"),
            ChatProviderConfig::mock("second", "second response"),
        ],
        Some("missing".to_string()),
    )
    .expect("manager should build");

    assert_eq!(manager.default_chat_provider_id(), Some("first"));
}

#[test]
fn manager_builds_openai_compatible_provider_aliases() {
    let registry = ProviderRegistry::with_builtin_chat_providers();

    for provider_type in OPENAI_COMPATIBLE_CHAT_PROVIDER_TYPES {
        let provider_id = format!("{provider_type}-provider");
        let manager = ProviderManager::from_chat_configs(
            &registry,
            vec![ChatProviderConfig::openai_compatible_with_type(
                *provider_type,
                provider_id.clone(),
                "http://127.0.0.1:1",
                "test-model",
            )],
            Some(provider_id.clone()),
        )
        .expect("OpenAI-compatible alias should build");

        assert_eq!(manager.chat_provider_count(), 1);
        assert_eq!(
            manager.default_chat_provider_id(),
            Some(provider_id.as_str())
        );
    }
}

#[tokio::test]
async fn manager_routes_chat_request_to_requested_provider() {
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![
            ChatProviderConfig::mock("primary", "primary response"),
            ChatProviderConfig::mock("secondary", "secondary response"),
        ],
        Some("primary".to_string()),
    )
    .expect("manager should build");

    let selected = manager
        .chat(ChatRequest::new("hello", "session").with_provider_id("secondary"))
        .await
        .expect("requested provider should respond");
    assert_eq!(selected.chain.plain_text(), "secondary response");

    let fallback = manager
        .chat(ChatRequest::new("hello", "session"))
        .await
        .expect("default provider should respond");
    assert_eq!(fallback.chain.plain_text(), "primary response");

    let missing = manager
        .chat(ChatRequest::new("hello", "session").with_provider_id("missing"))
        .await
        .expect_err("missing requested provider should fail");
    assert!(missing.to_string().contains("missing"));
}

#[tokio::test]
async fn openrouter_alias_adds_astrbot_default_headers() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"choices":[{"message":{"role":"assistant","content":"ok"}}]}"#,
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![
            ChatProviderConfig::openai_compatible_with_type(
                OPENROUTER_CHAT_PROVIDER_TYPE,
                "openrouter",
                base_url,
                "openrouter-model",
            )
            .with_api_key("test-key"),
        ],
        Some("openrouter".to_string()),
    )
    .expect("OpenRouter alias should build");

    let response = manager
        .chat(ChatRequest::new("hello", "session"))
        .await
        .expect("OpenRouter alias should use OpenAI-compatible request");

    assert_eq!(response.chain.plain_text(), "ok");
    let request = captured.lock().await.clone();
    assert!(request.contains(r#""model":"openrouter-model""#));
    assert!(has_header(
        &request,
        "HTTP-Referer",
        "https://github.com/AstrBotDevs/AstrBot"
    ));
    assert!(has_header(&request, "X-TITLE", "AstrBot"));
}

#[tokio::test]
async fn aihubmix_alias_adds_astrbot_default_header() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"choices":[{"message":{"role":"assistant","content":"ok"}}]}"#,
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_chat_providers();
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![ChatProviderConfig::openai_compatible_with_type(
            AIHUBMIX_CHAT_PROVIDER_TYPE,
            "aihubmix",
            base_url,
            "aihubmix-model",
        )],
        Some("aihubmix".to_string()),
    )
    .expect("AIHubMix alias should build");

    let response = manager
        .chat(ChatRequest::new("hello", "session"))
        .await
        .expect("AIHubMix alias should use OpenAI-compatible request");

    assert_eq!(response.chain.plain_text(), "ok");
    let request = captured.lock().await.clone();
    assert!(request.contains(r#""model":"aihubmix-model""#));
    assert!(has_header(&request, "APP-Code", "KRLC5702"));
}
