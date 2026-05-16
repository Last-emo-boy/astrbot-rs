use std::time::Duration;

use astrbot_platform::WEBCHAT_PLATFORM_TYPE;
use astrbot_provider::{
    ChatProviderConfig, EmbeddingProvider, EmbeddingProviderConfig, EmbeddingRequest,
    RerankProvider, RerankProviderConfig, RerankRequest, SpeechToTextProvider,
    SpeechToTextProviderConfig, SpeechToTextRequest, TextToSpeechProvider,
    TextToSpeechProviderConfig, TextToSpeechRequest,
};

use crate::{
    AstrbotRuntime, RuntimeChatProviderConfig, RuntimeConfig, RuntimeEmbeddingProviderConfig,
    RuntimeRerankProviderConfig, RuntimeSpeechToTextProviderConfig,
    RuntimeTextToSpeechProviderConfig,
};

#[tokio::test]
async fn runtime_builds_non_chat_provider_buckets_from_config() {
    let config = RuntimeConfig {
        chat_providers: Vec::new(),
        default_speech_to_text_provider_id: Some("stt-secondary".to_string()),
        speech_to_text_providers: vec![
            RuntimeSpeechToTextProviderConfig::mock("stt-primary", "primary transcript"),
            RuntimeSpeechToTextProviderConfig::mock("stt-secondary", "secondary transcript"),
        ],
        default_text_to_speech_provider_id: Some("tts-secondary".to_string()),
        text_to_speech_providers: vec![
            RuntimeTextToSpeechProviderConfig::mock("tts-primary", "primary.wav"),
            RuntimeTextToSpeechProviderConfig::mock("tts-secondary", "secondary.wav")
                .with_streaming(true),
        ],
        default_embedding_provider_id: Some("embedding-primary".to_string()),
        embedding_providers: vec![RuntimeEmbeddingProviderConfig::mock("embedding-primary", 4)],
        default_rerank_provider_id: Some("rerank-primary".to_string()),
        rerank_providers: vec![RuntimeRerankProviderConfig::mock("rerank-primary", 2)],
        ..RuntimeConfig::default()
    };
    let runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");
    let manager = runtime.provider_manager();

    assert_eq!(manager.chat_provider_count(), 0);
    assert_eq!(manager.speech_to_text_provider_count(), 2);
    assert_eq!(
        manager.default_speech_to_text_provider_id(),
        Some("stt-secondary")
    );
    assert_eq!(manager.text_to_speech_provider_count(), 2);
    assert_eq!(
        manager.default_text_to_speech_provider_id(),
        Some("tts-secondary")
    );
    assert!(manager.supports_text_to_speech_streaming());
    assert_eq!(manager.embedding_provider_count(), 1);
    assert_eq!(
        manager.default_embedding_provider_id(),
        Some("embedding-primary")
    );
    assert_eq!(manager.rerank_provider_count(), 1);
    assert_eq!(manager.default_rerank_provider_id(), Some("rerank-primary"));

    let transcript = manager
        .transcribe(SpeechToTextRequest::new("sample.wav"))
        .await
        .expect("default STT provider should respond");
    assert_eq!(transcript.text, "secondary transcript");

    let audio = manager
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect("default TTS provider should respond");
    assert_eq!(audio.audio_path, "secondary.wav");

    let embedding = manager
        .embed(EmbeddingRequest::new("hello"))
        .await
        .expect("default embedding provider should respond");
    assert_eq!(embedding.dimension(), Some(4));

    let reranked = manager
        .rerank(RerankRequest::new("query", ["doc-a", "doc-b"]).with_top_n(1))
        .await
        .expect("default rerank provider should respond");
    assert_eq!(reranked.results.len(), 1);
}

#[test]
fn runtime_config_defaults_include_disabled_webchat_server() {
    let config = RuntimeConfig::default();

    assert!(!config.whitelist_policy.enabled);
    assert_eq!(
        config.whitelist_policy.bypass_platform_ids,
        vec![WEBCHAT_PLATFORM_TYPE.to_string()]
    );
    assert!(config.session_status.disabled_sessions.is_empty());
    assert!(!config.rate_limit.enabled);
    assert!(config.content_safety.internal_keywords.enabled);
    assert!(
        config
            .content_safety
            .internal_keywords
            .extra_keywords
            .is_empty()
    );
    assert!(config.provider_fallback.enabled);
    assert!(!config.provider_fallback.require_wake);
    assert!(config.provider_fallback.error_message.is_some());
    assert!(config.result_decorate.reply_prefix.is_none());
    assert!(!config.result_decorate.only_llm_result);
    assert!(config.state_policy.preserve_provider_preference_on_restart);
    assert!(config.default_speech_to_text_provider_id.is_none());
    assert!(config.speech_to_text_providers.is_empty());
    assert!(config.default_text_to_speech_provider_id.is_none());
    assert!(config.text_to_speech_providers.is_empty());
    assert!(config.default_embedding_provider_id.is_none());
    assert!(config.embedding_providers.is_empty());
    assert!(config.default_rerank_provider_id.is_none());
    assert!(config.rerank_providers.is_empty());
    assert!(!config.webchat_server.enabled);
    assert_eq!(config.webchat_server.platform_id, WEBCHAT_PLATFORM_TYPE);
    assert_eq!(config.webchat_server.host, "127.0.0.1");
    assert_eq!(config.webchat_server.port, 6185);
}

#[test]
fn runtime_chat_provider_config_preserves_openai_compatible_aliases() {
    let config = RuntimeChatProviderConfig::openai_compatible_with_type(
        astrbot_provider::OPENROUTER_CHAT_PROVIDER_TYPE,
        "openrouter",
        "https://openrouter.example/v1",
        "openrouter-model",
    )
    .with_api_key("test-key")
    .with_timeout_secs(42);

    let provider_config = ChatProviderConfig::from(config);

    assert_eq!(
        provider_config.provider_type,
        astrbot_provider::OPENROUTER_CHAT_PROVIDER_TYPE
    );
    assert_eq!(provider_config.id, "openrouter");
    assert_eq!(
        provider_config.api_base.as_deref(),
        Some("https://openrouter.example/v1")
    );
    assert_eq!(provider_config.model.as_deref(), Some("openrouter-model"));
    assert_eq!(provider_config.api_key.as_deref(), Some("test-key"));
    assert_eq!(provider_config.timeout, Duration::from_secs(42));
}

#[test]
fn runtime_chat_provider_config_maps_anthropic_provider() {
    let config = RuntimeChatProviderConfig::anthropic(
        "anthropic",
        "https://anthropic.example",
        "claude-test",
    )
    .with_api_key("test-key")
    .with_timeout_secs(30);

    let provider_config = ChatProviderConfig::from(config);

    assert_eq!(
        provider_config.provider_type,
        astrbot_provider::ANTHROPIC_CHAT_PROVIDER_TYPE
    );
    assert_eq!(provider_config.id, "anthropic");
    assert_eq!(
        provider_config.api_base.as_deref(),
        Some("https://anthropic.example")
    );
    assert_eq!(provider_config.model.as_deref(), Some("claude-test"));
    assert_eq!(provider_config.api_key.as_deref(), Some("test-key"));
    assert_eq!(provider_config.timeout, Duration::from_secs(30));
}

#[test]
fn runtime_chat_provider_config_maps_gemini_provider() {
    let config = RuntimeChatProviderConfig::google_genai(
        "gemini",
        "https://generativelanguage.example",
        "gemini-test",
    )
    .with_api_key("test-key")
    .with_timeout_secs(25);

    let provider_config = ChatProviderConfig::from(config);

    assert_eq!(
        provider_config.provider_type,
        astrbot_provider::GOOGLE_GENAI_CHAT_PROVIDER_TYPE
    );
    assert_eq!(provider_config.id, "gemini");
    assert_eq!(
        provider_config.api_base.as_deref(),
        Some("https://generativelanguage.example")
    );
    assert_eq!(provider_config.model.as_deref(), Some("gemini-test"));
    assert_eq!(provider_config.api_key.as_deref(), Some("test-key"));
    assert_eq!(provider_config.timeout, Duration::from_secs(25));
}

#[test]
fn runtime_non_chat_provider_configs_map_concrete_types() {
    let stt = RuntimeSpeechToTextProviderConfig::openai(
        "openai-stt",
        "https://openai.example/v1",
        "whisper-test",
    )
    .with_api_key("stt-key")
    .with_timeout_secs(40);
    let stt_config = SpeechToTextProviderConfig::from(stt);
    assert_eq!(
        stt_config.provider_type,
        astrbot_provider::OPENAI_SPEECH_TO_TEXT_PROVIDER_TYPE
    );
    assert_eq!(stt_config.api_key.as_deref(), Some("stt-key"));
    assert_eq!(stt_config.timeout, Duration::from_secs(40));

    let tts = RuntimeTextToSpeechProviderConfig::gemini(
        "gemini-tts",
        "https://generativelanguage.example",
        "gemini-tts-test",
    )
    .with_api_key("tts-key")
    .with_voice("Kore")
    .with_timeout_secs(15);
    let tts_config = TextToSpeechProviderConfig::from(tts);
    assert_eq!(
        tts_config.provider_type,
        astrbot_provider::GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE
    );
    assert_eq!(tts_config.api_key.as_deref(), Some("tts-key"));
    assert_eq!(tts_config.voice.as_deref(), Some("Kore"));
    assert_eq!(tts_config.timeout, Duration::from_secs(15));

    let volcengine = RuntimeTextToSpeechProviderConfig::volcengine(
        "volcengine-tts",
        "https://openspeech.example/api/v1/tts",
    )
    .with_api_key("volcengine-key")
    .with_voice("BV700_streaming")
    .with_option("appid", "test-appid")
    .with_option("volcengine_cluster", "volcano-icl")
    .with_option("volcengine_speed_ratio", "1.2");
    let volcengine_config = TextToSpeechProviderConfig::from(volcengine);
    assert_eq!(
        volcengine_config.provider_type,
        astrbot_provider::VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE
    );
    assert_eq!(volcengine_config.api_key.as_deref(), Some("volcengine-key"));
    assert_eq!(volcengine_config.voice.as_deref(), Some("BV700_streaming"));
    assert_eq!(
        volcengine_config.provider_options.get("appid"),
        Some(&"test-appid".to_string())
    );
    assert_eq!(
        volcengine_config.provider_options.get("volcengine_cluster"),
        Some(&"volcano-icl".to_string())
    );

    let minimax = RuntimeTextToSpeechProviderConfig::minimax(
        "minimax-tts",
        "https://minimax.example/v1/t2a_v2",
        "speech-02-hd",
    )
    .with_api_key("minimax-key")
    .with_voice("female-qn-qingse")
    .with_option("minimax-group-id", "group-1")
    .with_option("minimax-langboost", "Chinese")
    .with_option("minimax-voice-speed", "1.2");
    let minimax_config = TextToSpeechProviderConfig::from(minimax);
    assert_eq!(
        minimax_config.provider_type,
        astrbot_provider::MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE
    );
    assert_eq!(minimax_config.api_key.as_deref(), Some("minimax-key"));
    assert_eq!(minimax_config.voice.as_deref(), Some("female-qn-qingse"));
    assert_eq!(
        minimax_config.provider_options.get("minimax-group-id"),
        Some(&"group-1".to_string())
    );

    let gsvi = RuntimeTextToSpeechProviderConfig::gsvi("gsvi-tts", "http://127.0.0.1:5000")
        .with_voice("mika")
        .with_option("emotion", "happy");
    let gsvi_config = TextToSpeechProviderConfig::from(gsvi);
    assert_eq!(
        gsvi_config.provider_type,
        astrbot_provider::GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE
    );
    assert_eq!(gsvi_config.voice.as_deref(), Some("mika"));
    assert_eq!(
        gsvi_config.provider_options.get("emotion"),
        Some(&"happy".to_string())
    );

    let embedding = RuntimeEmbeddingProviderConfig::openai(
        "openai-embedding",
        "https://openai.example/v1",
        "embedding-test",
    )
    .with_api_key("embedding-key")
    .with_dimensions(64);
    let embedding_config = EmbeddingProviderConfig::from(embedding);
    assert_eq!(
        embedding_config.provider_type,
        astrbot_provider::OPENAI_EMBEDDING_PROVIDER_TYPE
    );
    assert_eq!(embedding_config.api_key.as_deref(), Some("embedding-key"));
    assert_eq!(embedding_config.dimensions, Some(64));

    let rerank = RuntimeRerankProviderConfig::xinference(
        "xinference-rerank",
        "http://127.0.0.1:9997",
        "bge-reranker",
    )
    .with_api_key("rerank-key")
    .with_launch_model_if_not_running(true);
    let rerank_config = RerankProviderConfig::from(rerank);
    assert_eq!(
        rerank_config.provider_type,
        astrbot_provider::XINFERENCE_RERANK_PROVIDER_TYPE
    );
    assert_eq!(rerank_config.api_key.as_deref(), Some("rerank-key"));
    assert!(rerank_config.launch_model_if_not_running);
}
