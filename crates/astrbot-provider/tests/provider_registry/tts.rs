use super::*;

#[tokio::test]
async fn manager_builds_enabled_text_to_speech_providers_and_selects_default() {
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_text_to_speech_configs(
        &registry,
        vec![
            TextToSpeechProviderConfig::mock("disabled", "disabled.wav").disabled(),
            TextToSpeechProviderConfig::mock("primary", "primary.wav"),
            TextToSpeechProviderConfig::mock("secondary", "secondary.wav").with_streaming(true),
        ],
        Some("secondary".to_string()),
    )
    .expect("text-to-speech manager should build");

    assert_eq!(manager.text_to_speech_provider_count(), 2);
    assert_eq!(
        manager.default_text_to_speech_provider_id(),
        Some("secondary")
    );
    assert!(manager.supports_streaming());

    let response = manager
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect("default text-to-speech provider should respond");

    assert_eq!(response.audio_path, "secondary.wav");
}

#[tokio::test]
async fn manager_routes_text_to_speech_request_to_requested_provider() {
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_text_to_speech_configs(
        &registry,
        vec![
            TextToSpeechProviderConfig::mock("primary", "primary.wav"),
            TextToSpeechProviderConfig::mock("secondary", "secondary.wav"),
        ],
        Some("primary".to_string()),
    )
    .expect("text-to-speech manager should build");

    let selected = manager
        .synthesize(TextToSpeechRequest::new("hello").with_provider_id("secondary"))
        .await
        .expect("requested text-to-speech provider should respond");
    assert_eq!(selected.audio_path, "secondary.wav");

    let fallback = manager
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect("default text-to-speech provider should respond");
    assert_eq!(fallback.audio_path, "primary.wav");

    let missing = manager
        .synthesize(TextToSpeechRequest::new("hello").with_provider_id("missing"))
        .await
        .expect_err("missing requested text-to-speech provider should fail");
    assert!(missing.to_string().contains("missing"));
}

#[tokio::test]
async fn manager_builds_openai_text_to_speech_provider_from_registry() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once("200 OK", "audio/wav", "audio-bytes", captured.clone()).await;
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_text_to_speech_configs(
        &registry,
        vec![
            TextToSpeechProviderConfig::openai("openai-tts", base_url, "tts-1")
                .with_api_key("test-key")
                .with_voice("verse"),
        ],
        Some("openai-tts".to_string()),
    )
    .expect("text-to-speech manager should build");

    let response = manager
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect("OpenAI text-to-speech provider should respond");

    let audio = GeneratedAudioFile::new(response.audio_path);
    assert_eq!(audio.read(), b"audio-bytes");

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /v1/audio/speech HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains(r#""model":"tts-1""#));
    assert!(request.contains(r#""voice":"verse""#));
    assert!(request.contains(r#""input":"hello""#));
    assert!(request.contains(r#""response_format":"wav""#));
}

#[test]
fn manager_builds_gemini_text_to_speech_provider_from_registry() {
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_text_to_speech_configs(
        &registry,
        vec![
            TextToSpeechProviderConfig::gemini(
                "gemini-tts",
                "https://generativelanguage.googleapis.com",
                "gemini-2.5-flash-preview-tts",
            )
            .with_api_key("test-key")
            .with_voice("Kore"),
        ],
        Some("gemini-tts".to_string()),
    )
    .expect("text-to-speech manager should build");

    assert_eq!(manager.text_to_speech_provider_count(), 1);
    assert_eq!(
        manager.default_text_to_speech_provider_id(),
        Some("gemini-tts")
    );
}

#[tokio::test]
async fn manager_builds_volcengine_text_to_speech_provider_from_registry() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"data":"YXVkaW8="}"#,
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_text_to_speech_configs(
        &registry,
        vec![
            TextToSpeechProviderConfig::volcengine("volcengine-tts", base_url)
                .with_api_key("test-key")
                .with_option("appid", "test-appid")
                .with_option("volcengine_cluster", "volcano-icl")
                .with_option("volcengine_speed_ratio", "1.1")
                .with_voice("BV700_streaming"),
        ],
        Some("volcengine-tts".to_string()),
    )
    .expect("text-to-speech manager should build");

    let response = manager
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect("Volcengine text-to-speech provider should respond");

    let audio = GeneratedAudioFile::new(response.audio_path);
    assert_eq!(audio.read(), b"audio");

    let request = captured.lock().await.clone();
    assert!(request.contains("authorization: Bearer; test-key"));
    assert!(request.contains(r#""appid":"test-appid""#));
    assert!(request.contains(r#""cluster":"volcano-icl""#));
    assert!(request.contains(r#""voice_type":"BV700_streaming""#));
    assert!(request.contains(r#""speed_ratio":1.1"#));
    assert!(request.contains(r#""text":"hello""#));
}

#[tokio::test]
async fn manager_builds_minimax_text_to_speech_provider_from_registry() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "text/event-stream",
        "data: {\"data\":{\"audio\":\"617564696f\"}}\n\n",
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_text_to_speech_configs(
        &registry,
        vec![
            TextToSpeechProviderConfig::minimax(
                "minimax-tts",
                format!("{base_url}/v1/t2a_v2"),
                "speech-02-hd",
            )
            .with_api_key("test-key")
            .with_option("minimax-group-id", "group-1")
            .with_option("minimax-langboost", "Chinese")
            .with_option("minimax-voice-speed", "1.1")
            .with_voice("female-qn-qingse"),
        ],
        Some("minimax-tts".to_string()),
    )
    .expect("text-to-speech manager should build");

    let response = manager
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect("MiniMax text-to-speech provider should respond");

    let audio = GeneratedAudioFile::new(response.audio_path);
    assert_eq!(audio.read(), b"audio");

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /v1/t2a_v2?GroupId=group-1 HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains(r#""model":"speech-02-hd""#));
    assert!(request.contains(r#""language_boost":"Chinese""#));
    assert!(request.contains(r#""voice_id":"female-qn-qingse""#));
    assert!(request.contains(r#""speed":1.1"#));
    assert!(request.contains(r#""text":"hello""#));
}

#[tokio::test]
async fn manager_builds_gsvi_text_to_speech_provider_from_registry() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once("200 OK", "audio/wav", "audio", captured.clone()).await;
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_text_to_speech_configs(
        &registry,
        vec![
            TextToSpeechProviderConfig::gsvi("gsvi-tts", base_url)
                .with_voice("mika")
                .with_option("emotion", "happy"),
        ],
        Some("gsvi-tts".to_string()),
    )
    .expect("text-to-speech manager should build");

    let response = manager
        .synthesize(TextToSpeechRequest::new("hello world"))
        .await
        .expect("GSVI text-to-speech provider should respond");

    let audio = GeneratedAudioFile::new(response.audio_path);
    assert_eq!(audio.read(), b"audio");

    let request = captured.lock().await.clone();
    assert!(request.starts_with("GET /tts?"));
    assert!(request.contains("text=hello%20world"));
    assert!(request.contains("character=mika"));
    assert!(request.contains("emotion=happy"));
}

#[tokio::test]
async fn manager_builds_gsv_selfhost_text_to_speech_provider_from_registry() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![
            TestResponse::bytes("200 OK", "text/plain", b"ok".to_vec()),
            TestResponse::bytes("200 OK", "audio/wav", b"audio".to_vec()),
        ],
        captured.clone(),
    )
    .await;
    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_text_to_speech_configs(
        &registry,
        vec![
            TextToSpeechProviderConfig::gsv_selfhost("gsv-selfhost", base_url)
                .with_option("gpt_weights_path", "C:/models/gpt.ckpt")
                .with_option("gsv_prompt_text", "ref text")
                .with_option("gsv_text_lang", "zh")
                .with_header("x-gsv", "yes"),
        ],
        Some("gsv-selfhost".to_string()),
    )
    .expect("text-to-speech manager should build");

    let response = manager
        .synthesize(TextToSpeechRequest::new("hello world"))
        .await
        .expect("GSV selfhost text-to-speech provider should respond");

    let audio = GeneratedAudioFile::new(response.audio_path);
    assert_eq!(audio.read(), b"audio");

    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0]
            .starts_with("GET /set_gpt_weights?weights_path=C%3A%2Fmodels%2Fgpt.ckpt HTTP/1.1")
    );
    assert!(has_header(&requests[0], "x-gsv", "yes"));
    assert!(requests[1].starts_with("GET /tts?"));
    assert!(requests[1].contains("prompt_text=ref%20text"));
    assert!(requests[1].contains("text_lang=zh"));
    assert!(requests[1].contains("text=hello%20world"));
}

#[tokio::test]
async fn manager_builds_commercial_text_to_speech_providers_from_registry() {
    let azure_token_request = Arc::new(Mutex::new(String::new()));
    let azure_token_url = format!(
        "{}/token",
        serve_once("200 OK", "text/plain", "token", azure_token_request).await
    );
    let azure_tts_request = Arc::new(Mutex::new(String::new()));
    let azure_tts_url = format!(
        "{}/tts",
        serve_once(
            "200 OK",
            "audio/wav",
            "azure-audio",
            azure_tts_request.clone()
        )
        .await
    );
    let edge_requests = Arc::new(Mutex::new(Vec::new()));
    let edge_base = serve_sequence(
        vec![TestResponse::bytes(
            "200 OK",
            "audio/wav",
            b"edge-audio".to_vec(),
        )],
        edge_requests.clone(),
    )
    .await;
    let dashscope_audio = base64::engine::general_purpose::STANDARD.encode(b"dashscope-audio");
    let dashscope_requests = Arc::new(Mutex::new(Vec::new()));
    let dashscope_base = serve_sequence(
        vec![TestResponse::json(
            "200 OK",
            &format!(r#"{{"output":{{"audio":{{"data":"{dashscope_audio}"}}}}}}"#),
        )],
        dashscope_requests.clone(),
    )
    .await;
    let fishaudio_requests = Arc::new(Mutex::new(Vec::new()));
    let fishaudio_base = serve_sequence(
        vec![TestResponse::bytes(
            "200 OK",
            "audio/wav",
            b"fish-audio".to_vec(),
        )],
        fishaudio_requests.clone(),
    )
    .await;
    let genie_requests = Arc::new(Mutex::new(Vec::new()));
    let genie_base = serve_sequence(
        vec![TestResponse::bytes(
            "200 OK",
            "audio/wav",
            b"genie-audio".to_vec(),
        )],
        genie_requests.clone(),
    )
    .await;

    let registry = ProviderRegistry::with_builtin_providers();
    let manager = ProviderManager::from_text_to_speech_configs(
        &registry,
        vec![
            TextToSpeechProviderConfig::azure("azure-tts", "a".repeat(32))
                .with_option("azure_tts_token_url", azure_token_url)
                .with_option("azure_tts_endpoint", azure_tts_url)
                .with_voice("zh-CN-XiaoxiaoNeural"),
            TextToSpeechProviderConfig::edge("edge-tts", edge_base)
                .with_option("edge-tts-voice", "zh-CN-XiaoxiaoNeural")
                .with_option("rate", "+10%"),
            TextToSpeechProviderConfig::dashscope(
                "dashscope-tts",
                dashscope_base,
                "qwen-tts-latest",
            )
            .with_api_key("dashscope-key")
            .with_voice("Cherry"),
            TextToSpeechProviderConfig::fishaudio("fishaudio-tts", format!("{fishaudio_base}/v1"))
                .with_api_key("fish-key")
                .with_option(
                    "fishaudio-tts-reference-id",
                    "626bb6d3f3364c9cbc3aa6a67300a664",
                ),
            TextToSpeechProviderConfig::genie("genie-tts", genie_base)
                .with_option("genie_language", "Japanese"),
        ],
        Some("genie-tts".to_string()),
    )
    .expect("commercial text-to-speech providers should build");

    assert_eq!(manager.text_to_speech_provider_count(), 5);
    assert!(manager.supports_text_to_speech_streaming());

    let cases = [
        ("azure-tts", b"azure-audio".as_slice()),
        ("edge-tts", b"edge-audio".as_slice()),
        ("dashscope-tts", b"dashscope-audio".as_slice()),
        ("fishaudio-tts", b"fish-audio".as_slice()),
        ("genie-tts", b"genie-audio".as_slice()),
    ];
    for (provider_id, expected_audio) in cases {
        let response = manager
            .synthesize(TextToSpeechRequest::new("hello").with_provider_id(provider_id))
            .await
            .expect("commercial TTS provider should synthesize audio");
        let audio = GeneratedAudioFile::new(response.audio_path);
        assert_eq!(audio.read(), expected_audio);
    }

    assert!(
        azure_tts_request
            .lock()
            .await
            .contains("zh-CN-XiaoxiaoNeural")
    );
    assert!(edge_requests.lock().await[0].contains(r#""rate":"+10%""#));
    assert!(dashscope_requests.lock().await[0].contains(r#""voice":"Cherry""#));
    assert!(
        fishaudio_requests.lock().await[0]
            .contains(r#""reference_id":"626bb6d3f3364c9cbc3aa6a67300a664""#)
    );
    assert!(genie_requests.lock().await[0].contains(r#""language":"Japanese""#));
}
