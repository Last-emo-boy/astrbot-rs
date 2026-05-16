use super::*;

#[tokio::test]
async fn manager_builds_enabled_speech_to_text_providers_and_selects_default() {
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_speech_to_text_configs(
        &registry,
        vec![
            SpeechToTextProviderConfig::mock("disabled", "disabled").disabled(),
            SpeechToTextProviderConfig::mock("primary", "primary transcript"),
            SpeechToTextProviderConfig::mock("secondary", "secondary transcript"),
        ],
        Some("secondary".to_string()),
    )
    .expect("speech-to-text manager should build");

    assert_eq!(manager.speech_to_text_provider_count(), 2);
    assert_eq!(
        manager.default_speech_to_text_provider_id(),
        Some("secondary")
    );

    let response = manager
        .transcribe(SpeechToTextRequest::new("sample.wav"))
        .await
        .expect("default speech-to-text provider should respond");

    assert_eq!(response.text, "secondary transcript");
}

#[tokio::test]
async fn manager_routes_speech_to_text_request_to_requested_provider() {
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_speech_to_text_configs(
        &registry,
        vec![
            SpeechToTextProviderConfig::mock("primary", "primary transcript"),
            SpeechToTextProviderConfig::mock("secondary", "secondary transcript"),
        ],
        Some("primary".to_string()),
    )
    .expect("speech-to-text manager should build");

    let selected = manager
        .transcribe(SpeechToTextRequest::new("sample.wav").with_provider_id("secondary"))
        .await
        .expect("requested speech-to-text provider should respond");
    assert_eq!(selected.text, "secondary transcript");

    let fallback = manager
        .transcribe(SpeechToTextRequest::new("sample.wav"))
        .await
        .expect("default speech-to-text provider should respond");
    assert_eq!(fallback.text, "primary transcript");

    let missing = manager
        .transcribe(SpeechToTextRequest::new("sample.wav").with_provider_id("missing"))
        .await
        .expect_err("missing requested speech-to-text provider should fail");
    assert!(missing.to_string().contains("missing"));
}

#[tokio::test]
async fn manager_builds_openai_speech_to_text_provider_from_registry() {
    let audio = TempAudioFile::wav("provider-registry-openai-stt", b"RIFF audio bytes");
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"text":"hello transcript"}"#,
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_speech_to_text_configs(
        &registry,
        vec![
            SpeechToTextProviderConfig::openai("openai-stt", base_url, "whisper-1")
                .with_api_key("test-key"),
        ],
        Some("openai-stt".to_string()),
    )
    .expect("speech-to-text manager should build");

    let response = manager
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect("OpenAI speech-to-text provider should respond");

    assert_eq!(response.text, "hello transcript");
    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /v1/audio/transcriptions HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains("whisper-1"));
    assert!(request.contains("RIFF audio bytes"));
}

#[test]
fn manager_builds_xinference_speech_to_text_provider_from_registry() {
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_speech_to_text_configs(
        &registry,
        vec![
            SpeechToTextProviderConfig::xinference(
                "xinference-stt",
                "http://127.0.0.1:9997",
                "whisper-large-v3",
            )
            .with_api_key("test-key")
            .with_launch_model_if_not_running(true),
        ],
        Some("xinference-stt".to_string()),
    )
    .expect("speech-to-text manager should build");

    assert_eq!(manager.speech_to_text_provider_count(), 1);
    assert_eq!(
        manager.default_speech_to_text_provider_id(),
        Some("xinference-stt")
    );
}
