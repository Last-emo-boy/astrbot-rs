use std::sync::Arc;

use crate::capability::ProviderModelDiscoverySupport;
use crate::constants::*;
use crate::factories::{
    OpenAiCompatiblePreset, build_anthropic_provider, build_azure_text_to_speech_provider,
    build_bailian_rerank_provider, build_dashscope_text_to_speech_provider,
    build_edge_text_to_speech_provider, build_fishaudio_text_to_speech_provider,
    build_gemini_embedding_provider, build_gemini_provider, build_gemini_text_to_speech_provider,
    build_genie_text_to_speech_provider, build_gsv_selfhost_text_to_speech_provider,
    build_gsvi_text_to_speech_provider, build_minimax_text_to_speech_provider,
    build_openai_compatible_provider, build_openai_embedding_provider,
    build_openai_speech_to_text_provider, build_openai_text_to_speech_provider,
    build_openai_whisper_selfhost_speech_to_text_provider,
    build_sensevoice_selfhost_speech_to_text_provider, build_vllm_rerank_provider,
    build_volcengine_text_to_speech_provider, build_xinference_rerank_provider,
    build_xinference_speech_to_text_provider, register_openai_compatible_alias,
};
use crate::{
    MockChatProvider, MockEmbeddingProvider, MockRerankProvider, MockSpeechToTextProvider,
    MockTextToSpeechProvider, default_model_candidates, model_discovery_support,
};

use super::ProviderRegistry;

pub(super) fn with_builtin_providers() -> ProviderRegistry {
    let mut registry = with_builtin_chat_providers();
    registry
        .register_speech_to_text_provider(MOCK_SPEECH_TO_TEXT_PROVIDER_TYPE, |config| {
            let text = config
                .mock_text
                .clone()
                .unwrap_or_else(|| "mock transcription".to_string());
            Ok(Arc::new(MockSpeechToTextProvider::new(text)))
        })
        .expect("built-in mock speech-to-text provider type should register once");
    registry
        .register_speech_to_text_provider(
            OPENAI_SPEECH_TO_TEXT_PROVIDER_TYPE,
            build_openai_speech_to_text_provider,
        )
        .expect("built-in OpenAI speech-to-text provider type should register once");
    registry
        .register_speech_to_text_provider(
            XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE,
            build_xinference_speech_to_text_provider,
        )
        .expect("built-in Xinference speech-to-text provider type should register once");
    registry
        .register_speech_to_text_provider(
            OPENAI_WHISPER_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE,
            build_openai_whisper_selfhost_speech_to_text_provider,
        )
        .expect("built-in selfhost Whisper speech-to-text provider type should register once");
    registry
        .register_speech_to_text_provider(
            SENSEVOICE_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE,
            build_sensevoice_selfhost_speech_to_text_provider,
        )
        .expect("built-in selfhost SenseVoice speech-to-text provider type should register once");
    registry
        .register_text_to_speech_provider(MOCK_TEXT_TO_SPEECH_PROVIDER_TYPE, |config| {
            let audio_path = config
                .mock_audio_path
                .clone()
                .unwrap_or_else(|| "mock.wav".to_string());
            Ok(Arc::new(
                MockTextToSpeechProvider::new(audio_path).with_streaming(config.supports_streaming),
            ))
        })
        .expect("built-in mock text-to-speech provider type should register once");
    registry
        .register_text_to_speech_provider(
            OPENAI_TEXT_TO_SPEECH_PROVIDER_TYPE,
            build_openai_text_to_speech_provider,
        )
        .expect("built-in OpenAI text-to-speech provider type should register once");
    registry
        .register_text_to_speech_provider(
            GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE,
            build_gemini_text_to_speech_provider,
        )
        .expect("built-in Gemini text-to-speech provider type should register once");
    registry
        .register_text_to_speech_provider(
            VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE,
            build_volcengine_text_to_speech_provider,
        )
        .expect("built-in Volcengine text-to-speech provider type should register once");
    registry
        .register_text_to_speech_provider(
            MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE,
            build_minimax_text_to_speech_provider,
        )
        .expect("built-in MiniMax text-to-speech provider type should register once");
    registry
        .register_text_to_speech_provider(
            GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE,
            build_gsvi_text_to_speech_provider,
        )
        .expect("built-in GSVI text-to-speech provider type should register once");
    registry
        .register_text_to_speech_provider(
            GSV_SELFHOST_TEXT_TO_SPEECH_PROVIDER_TYPE,
            build_gsv_selfhost_text_to_speech_provider,
        )
        .expect("built-in GSV selfhost text-to-speech provider type should register once");
    registry
        .register_text_to_speech_provider(
            AZURE_TEXT_TO_SPEECH_PROVIDER_TYPE,
            build_azure_text_to_speech_provider,
        )
        .expect("built-in Azure text-to-speech provider type should register once");
    registry
        .register_text_to_speech_provider(
            EDGE_TEXT_TO_SPEECH_PROVIDER_TYPE,
            build_edge_text_to_speech_provider,
        )
        .expect("built-in Edge text-to-speech provider type should register once");
    registry
        .register_text_to_speech_provider(
            DASHSCOPE_TEXT_TO_SPEECH_PROVIDER_TYPE,
            build_dashscope_text_to_speech_provider,
        )
        .expect("built-in Dashscope text-to-speech provider type should register once");
    registry
        .register_text_to_speech_provider(
            FISHAUDIO_TEXT_TO_SPEECH_PROVIDER_TYPE,
            build_fishaudio_text_to_speech_provider,
        )
        .expect("built-in FishAudio text-to-speech provider type should register once");
    registry
        .register_text_to_speech_provider(
            GENIE_TEXT_TO_SPEECH_PROVIDER_TYPE,
            build_genie_text_to_speech_provider,
        )
        .expect("built-in Genie text-to-speech provider type should register once");
    registry
        .register_embedding_provider(MOCK_EMBEDDING_PROVIDER_TYPE, |config| {
            let embedding = config
                .mock_embedding
                .clone()
                .unwrap_or_else(|| vec![0.0; config.dimensions.unwrap_or(3)]);
            Ok(Arc::new(MockEmbeddingProvider::new(embedding)))
        })
        .expect("built-in mock embedding provider type should register once");
    registry
        .register_embedding_provider(
            OPENAI_EMBEDDING_PROVIDER_TYPE,
            build_openai_embedding_provider,
        )
        .expect("built-in OpenAI embedding provider type should register once");
    registry
        .register_embedding_provider(
            GEMINI_EMBEDDING_PROVIDER_TYPE,
            build_gemini_embedding_provider,
        )
        .expect("built-in Gemini embedding provider type should register once");
    registry
        .register_rerank_provider(MOCK_RERANK_PROVIDER_TYPE, |config| {
            Ok(Arc::new(MockRerankProvider::new(
                config.mock_scores.clone().unwrap_or_default(),
            )))
        })
        .expect("built-in mock rerank provider type should register once");
    registry
        .register_rerank_provider(VLLM_RERANK_PROVIDER_TYPE, build_vllm_rerank_provider)
        .expect("built-in VLLM rerank provider type should register once");
    registry
        .register_rerank_provider(BAILIAN_RERANK_PROVIDER_TYPE, build_bailian_rerank_provider)
        .expect("built-in Bailian rerank provider type should register once");
    registry
        .register_rerank_provider(
            XINFERENCE_RERANK_PROVIDER_TYPE,
            build_xinference_rerank_provider,
        )
        .expect("built-in Xinference rerank provider type should register once");
    annotate_builtin_model_metadata(&mut registry);
    registry
}

pub(super) fn with_builtin_chat_providers() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry
        .register_chat_provider(MOCK_CHAT_PROVIDER_TYPE, |config| {
            let response = config
                .mock_response
                .clone()
                .unwrap_or_else(|| "mock response".to_string());
            Ok(Arc::new(MockChatProvider::new(response)))
        })
        .expect("built-in mock provider type should register once");
    registry
        .register_chat_provider(OPENAI_CHAT_PROVIDER_TYPE, |config| {
            build_openai_compatible_provider(config, OpenAiCompatiblePreset::Default)
        })
        .expect("built-in OpenAI provider type should register once");
    registry
        .register_chat_provider(ANTHROPIC_CHAT_PROVIDER_TYPE, build_anthropic_provider)
        .expect("built-in Anthropic provider type should register once");
    registry
        .register_chat_provider(GOOGLE_GENAI_CHAT_PROVIDER_TYPE, build_gemini_provider)
        .expect("built-in Gemini provider type should register once");
    register_openai_compatible_alias(
        &mut registry,
        ZHIPU_CHAT_PROVIDER_TYPE,
        OpenAiCompatiblePreset::Default,
    );
    register_openai_compatible_alias(
        &mut registry,
        GROQ_CHAT_PROVIDER_TYPE,
        OpenAiCompatiblePreset::Default,
    );
    register_openai_compatible_alias(
        &mut registry,
        XAI_CHAT_PROVIDER_TYPE,
        OpenAiCompatiblePreset::Default,
    );
    register_openai_compatible_alias(
        &mut registry,
        AIHUBMIX_CHAT_PROVIDER_TYPE,
        OpenAiCompatiblePreset::AiHubMix,
    );
    register_openai_compatible_alias(
        &mut registry,
        OPENROUTER_CHAT_PROVIDER_TYPE,
        OpenAiCompatiblePreset::OpenRouter,
    );
    annotate_builtin_model_metadata(&mut registry);
    registry
}

fn annotate_builtin_model_metadata(registry: &mut ProviderRegistry) {
    for provider_type in [
        MOCK_CHAT_PROVIDER_TYPE,
        OPENAI_CHAT_PROVIDER_TYPE,
        ANTHROPIC_CHAT_PROVIDER_TYPE,
        GOOGLE_GENAI_CHAT_PROVIDER_TYPE,
        ZHIPU_CHAT_PROVIDER_TYPE,
        GROQ_CHAT_PROVIDER_TYPE,
        XAI_CHAT_PROVIDER_TYPE,
        AIHUBMIX_CHAT_PROVIDER_TYPE,
        OPENROUTER_CHAT_PROVIDER_TYPE,
    ] {
        registry.set_provider_model_metadata(
            provider_type,
            model_discovery_support(provider_type),
            default_model_candidates(provider_type),
        );
    }

    for provider_type in [
        XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE,
        XINFERENCE_RERANK_PROVIDER_TYPE,
    ] {
        if registry.has_provider_adapter(provider_type) {
            registry.set_provider_model_metadata(
                provider_type,
                ProviderModelDiscoverySupport::Supported,
                Vec::new(),
            );
        }
    }
}
