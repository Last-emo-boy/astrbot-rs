use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use astrbot_provider::{
    AIHUBMIX_CHAT_PROVIDER_TYPE, ANTHROPIC_CHAT_PROVIDER_TYPE, BAILIAN_RERANK_PROVIDER_TYPE,
    ChatProvider, ChatProviderConfig, ChatRequest, ChatResponse, EmbeddingProvider,
    EmbeddingProviderConfig, EmbeddingRequest, EmbeddingResponse, GEMINI_EMBEDDING_PROVIDER_TYPE,
    GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE, GOOGLE_GENAI_CHAT_PROVIDER_TYPE,
    GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE, MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE,
    MOCK_CHAT_PROVIDER_TYPE, MOCK_EMBEDDING_PROVIDER_TYPE, MOCK_RERANK_PROVIDER_TYPE,
    MOCK_SPEECH_TO_TEXT_PROVIDER_TYPE, MOCK_TEXT_TO_SPEECH_PROVIDER_TYPE,
    OPENAI_COMPATIBLE_CHAT_PROVIDER_TYPES, OPENAI_EMBEDDING_PROVIDER_TYPE,
    OPENAI_SPEECH_TO_TEXT_PROVIDER_TYPE, OPENAI_TEXT_TO_SPEECH_PROVIDER_TYPE,
    OPENROUTER_CHAT_PROVIDER_TYPE, ProviderCapability, ProviderManager, ProviderManagerConfigSet,
    ProviderRegistry, RerankDocumentScore, RerankProvider, RerankProviderConfig, RerankRequest,
    RerankResponse, SpeechToTextProvider, SpeechToTextProviderConfig, SpeechToTextRequest,
    SpeechToTextResponse, TextToSpeechProvider, TextToSpeechProviderConfig, TextToSpeechRequest,
    TextToSpeechResponse, VLLM_RERANK_PROVIDER_TYPE, VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE,
    XINFERENCE_RERANK_PROVIDER_TYPE, XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE,
};
use async_trait::async_trait;
use tokio::sync::Mutex;

mod support;
use support::captured_request::has_header;
use support::http_server::serve_once;
use support::media_fixture::{GeneratedAudioFile, TempAudioFile};

#[path = "provider_registry/chat.rs"]
mod chat;
#[path = "provider_registry/embedding.rs"]
mod embedding;
#[path = "provider_registry/lifecycle.rs"]
mod lifecycle;
#[path = "provider_registry/rerank.rs"]
mod rerank;
#[path = "provider_registry/speech.rs"]
mod speech;
#[path = "provider_registry/tts.rs"]
mod tts;

struct TerminatingProvider {
    terminate_count: Arc<AtomicUsize>,
}

#[async_trait]
impl ChatProvider for TerminatingProvider {
    async fn chat(&self, _request: ChatRequest) -> astrbot_core::Result<ChatResponse> {
        Ok(ChatResponse::text("ok"))
    }

    async fn terminate(&self) -> astrbot_core::Result<()> {
        self.terminate_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct TerminatingSpeechToTextProvider {
    terminate_count: Arc<AtomicUsize>,
}

#[async_trait]
impl SpeechToTextProvider for TerminatingSpeechToTextProvider {
    async fn transcribe(
        &self,
        _request: SpeechToTextRequest,
    ) -> astrbot_core::Result<SpeechToTextResponse> {
        Ok(SpeechToTextResponse::new("ok"))
    }

    async fn terminate(&self) -> astrbot_core::Result<()> {
        self.terminate_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct TerminatingTextToSpeechProvider {
    terminate_count: Arc<AtomicUsize>,
}

#[async_trait]
impl TextToSpeechProvider for TerminatingTextToSpeechProvider {
    async fn synthesize(
        &self,
        _request: TextToSpeechRequest,
    ) -> astrbot_core::Result<TextToSpeechResponse> {
        Ok(TextToSpeechResponse::new("ok.wav"))
    }

    async fn terminate(&self) -> astrbot_core::Result<()> {
        self.terminate_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct TerminatingEmbeddingProvider {
    terminate_count: Arc<AtomicUsize>,
}

#[async_trait]
impl EmbeddingProvider for TerminatingEmbeddingProvider {
    async fn embed(&self, _request: EmbeddingRequest) -> astrbot_core::Result<EmbeddingResponse> {
        Ok(EmbeddingResponse::new(vec![vec![1.0]]))
    }

    async fn terminate(&self) -> astrbot_core::Result<()> {
        self.terminate_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct TerminatingRerankProvider {
    terminate_count: Arc<AtomicUsize>,
}

#[async_trait]
impl RerankProvider for TerminatingRerankProvider {
    async fn rerank(&self, _request: RerankRequest) -> astrbot_core::Result<RerankResponse> {
        Ok(RerankResponse::new(vec![RerankDocumentScore::new(0, 1.0)]))
    }

    async fn terminate(&self) -> astrbot_core::Result<()> {
        self.terminate_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
