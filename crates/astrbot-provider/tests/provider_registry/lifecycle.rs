use super::*;

#[tokio::test]
async fn manager_builds_all_capability_buckets_from_config_set() {
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_configs(
        &registry,
        ProviderManagerConfigSet {
            chat_providers: vec![ChatProviderConfig::mock("chat", "chat response")],
            default_chat_provider_id: Some("chat".to_string()),
            speech_to_text_providers: vec![SpeechToTextProviderConfig::mock("stt", "transcript")],
            default_speech_to_text_provider_id: Some("stt".to_string()),
            text_to_speech_providers: vec![TextToSpeechProviderConfig::mock("tts", "voice.wav")],
            default_text_to_speech_provider_id: Some("tts".to_string()),
            embedding_providers: vec![EmbeddingProviderConfig::mock("embedding", vec![0.0, 1.0])],
            default_embedding_provider_id: Some("embedding".to_string()),
            rerank_providers: vec![RerankProviderConfig::mock("rerank", vec![0.9, 0.2])],
            default_rerank_provider_id: Some("rerank".to_string()),
        },
    )
    .expect("provider manager should build all buckets");

    assert_eq!(manager.chat_provider_count(), 1);
    assert_eq!(manager.speech_to_text_provider_count(), 1);
    assert_eq!(manager.text_to_speech_provider_count(), 1);
    assert_eq!(manager.embedding_provider_count(), 1);
    assert_eq!(manager.rerank_provider_count(), 1);

    assert_eq!(
        manager
            .chat(ChatRequest::new("hello", "session"))
            .await
            .expect("chat provider should respond")
            .chain
            .plain_text(),
        "chat response"
    );
    assert_eq!(
        manager
            .transcribe(SpeechToTextRequest::new("audio.wav"))
            .await
            .expect("STT provider should respond")
            .text,
        "transcript"
    );
    assert_eq!(
        manager
            .synthesize(TextToSpeechRequest::new("hello"))
            .await
            .expect("TTS provider should respond")
            .audio_path,
        "voice.wav"
    );
    assert_eq!(
        manager
            .embed(EmbeddingRequest::new("hello"))
            .await
            .expect("embedding provider should respond")
            .dimension(),
        Some(2)
    );
    assert_eq!(
        manager
            .rerank(RerankRequest::new("query", ["first", "second"]))
            .await
            .expect("rerank provider should respond")
            .results[0]
            .index,
        0
    );
}

#[tokio::test]
async fn manager_terminates_configured_chat_providers() {
    let terminate_count = Arc::new(AtomicUsize::new(0));
    let provider_count = terminate_count.clone();
    let mut registry = ProviderRegistry::new();
    registry
        .register_chat_provider("terminating", move |_| {
            Ok(Arc::new(TerminatingProvider {
                terminate_count: provider_count.clone(),
            }))
        })
        .expect("custom provider should register");
    let manager = ProviderManager::from_chat_configs(
        &registry,
        vec![ChatProviderConfig {
            id: "provider-1".to_string(),
            provider_type: "terminating".to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout: std::time::Duration::from_secs(1),
            custom_headers: Default::default(),
            mock_response: None,
        }],
        Some("provider-1".to_string()),
    )
    .expect("manager should build");

    manager.terminate().await.expect("manager should terminate");

    assert_eq!(terminate_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn manager_terminates_configured_speech_to_text_providers() {
    let terminate_count = Arc::new(AtomicUsize::new(0));
    let provider_count = terminate_count.clone();
    let mut registry = ProviderRegistry::new();
    registry
        .register_speech_to_text_provider("terminating_stt", move |_| {
            Ok(Arc::new(TerminatingSpeechToTextProvider {
                terminate_count: provider_count.clone(),
            }))
        })
        .expect("custom speech-to-text provider should register");
    let manager = ProviderManager::from_speech_to_text_configs(
        &registry,
        vec![SpeechToTextProviderConfig {
            id: "provider-1".to_string(),
            provider_type: "terminating_stt".to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout: std::time::Duration::from_secs(1),
            custom_headers: Default::default(),
            provider_options: Default::default(),
            mock_text: None,
            launch_model_if_not_running: false,
        }],
        Some("provider-1".to_string()),
    )
    .expect("speech-to-text manager should build");

    manager.terminate().await.expect("manager should terminate");

    assert_eq!(terminate_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn manager_terminates_configured_text_to_speech_providers() {
    let terminate_count = Arc::new(AtomicUsize::new(0));
    let provider_count = terminate_count.clone();
    let mut registry = ProviderRegistry::new();
    registry
        .register_text_to_speech_provider("terminating_tts", move |_| {
            Ok(Arc::new(TerminatingTextToSpeechProvider {
                terminate_count: provider_count.clone(),
            }))
        })
        .expect("custom text-to-speech provider should register");
    let manager = ProviderManager::from_text_to_speech_configs(
        &registry,
        vec![TextToSpeechProviderConfig {
            id: "provider-1".to_string(),
            provider_type: "terminating_tts".to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout: std::time::Duration::from_secs(1),
            custom_headers: Default::default(),
            mock_audio_path: None,
            supports_streaming: false,
            voice: None,
            provider_options: Default::default(),
        }],
        Some("provider-1".to_string()),
    )
    .expect("text-to-speech manager should build");

    manager.terminate().await.expect("manager should terminate");

    assert_eq!(terminate_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn manager_terminates_configured_embedding_providers() {
    let terminate_count = Arc::new(AtomicUsize::new(0));
    let provider_count = terminate_count.clone();
    let mut registry = ProviderRegistry::new();
    registry
        .register_embedding_provider("terminating_embedding", move |_| {
            Ok(Arc::new(TerminatingEmbeddingProvider {
                terminate_count: provider_count.clone(),
            }))
        })
        .expect("custom embedding provider should register");
    let manager = ProviderManager::from_embedding_configs(
        &registry,
        vec![EmbeddingProviderConfig {
            id: "provider-1".to_string(),
            provider_type: "terminating_embedding".to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout: std::time::Duration::from_secs(1),
            custom_headers: Default::default(),
            dimensions: None,
            mock_embedding: None,
        }],
        Some("provider-1".to_string()),
    )
    .expect("embedding manager should build");

    manager.terminate().await.expect("manager should terminate");

    assert_eq!(terminate_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn manager_terminates_configured_rerank_providers() {
    let terminate_count = Arc::new(AtomicUsize::new(0));
    let provider_count = terminate_count.clone();
    let mut registry = ProviderRegistry::new();
    registry
        .register_rerank_provider("terminating_rerank", move |_| {
            Ok(Arc::new(TerminatingRerankProvider {
                terminate_count: provider_count.clone(),
            }))
        })
        .expect("custom rerank provider should register");
    let manager = ProviderManager::from_rerank_configs(
        &registry,
        vec![RerankProviderConfig {
            id: "provider-1".to_string(),
            provider_type: "terminating_rerank".to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout: std::time::Duration::from_secs(1),
            custom_headers: Default::default(),
            mock_scores: None,
            launch_model_if_not_running: false,
        }],
        Some("provider-1".to_string()),
    )
    .expect("rerank manager should build");

    manager.terminate().await.expect("manager should terminate");

    assert_eq!(terminate_count.load(Ordering::SeqCst), 1);
}
