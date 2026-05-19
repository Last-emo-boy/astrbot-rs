mod anthropic;
mod audio;
mod bailian_rerank;
mod capability;
mod chat;
mod commercial_tts;
mod config;
mod constants;
mod embedding;
mod factories;
mod gemini;
mod gemini_embedding;
mod gemini_tts;
mod gsv_selfhost_tts;
mod gsvi_tts;
mod http;
mod manager;
mod media;
mod minimax_tts;
mod mock;
mod model_discovery;
mod model_resolver;
mod openai_compatible;
mod openai_embedding;
mod openai_stt;
mod openai_tts;
mod protocol;
mod registry;
mod rerank;
mod response;
mod selection;
mod selfhost_stt;
mod speech;
mod streaming;
mod tts;
mod vllm_rerank;
mod volcengine_tts;
mod xinference_rerank;
mod xinference_stt;

pub use anthropic::{AnthropicConfig, AnthropicProvider};
pub use audio::{
    AudioConversionCommand, AudioConversionCommandRequest, AudioConversionRequest, AudioFormat,
    AudioInputLoader, AudioMediaConverter, AudioTranscodeTarget, FfmpegAudioMediaConverter,
    FfmpegCommandRunner, UnsupportedAudioMediaConverter, detect_audio_conversion_requirement,
};
pub use bailian_rerank::{BailianRerankConfig, BailianRerankProvider};
pub use capability::{
    ProviderAdapterMetadata, ProviderCapability, ProviderModelDiscoverySupport, ProviderModelInfo,
};
pub use chat::{ChatProvider, ChatRequest, ChatResponse};
pub use commercial_tts::{
    AzureCommercialTextToSpeechProvider, AzureOttsTextToSpeechConfig, AzureTextToSpeechConfig,
    AzureTextToSpeechProvider, DashscopeTextToSpeechConfig, DashscopeTextToSpeechProvider,
    DashscopeTtsMode, EdgeTextToSpeechConfig, EdgeTextToSpeechProvider,
    FishAudioTextToSpeechConfig, FishAudioTextToSpeechProvider, GenieTextToSpeechConfig,
    GenieTextToSpeechProvider,
};
pub use config::{
    ChatProviderConfig, EmbeddingProviderConfig, RerankProviderConfig, SpeechToTextProviderConfig,
    TextToSpeechProviderConfig,
};
pub use constants::{
    AIHUBMIX_CHAT_PROVIDER_TYPE, ANTHROPIC_CHAT_PROVIDER_TYPE, AZURE_TEXT_TO_SPEECH_PROVIDER_TYPE,
    BAILIAN_RERANK_PROVIDER_TYPE, DASHSCOPE_TEXT_TO_SPEECH_PROVIDER_TYPE,
    EDGE_TEXT_TO_SPEECH_PROVIDER_TYPE, FISHAUDIO_TEXT_TO_SPEECH_PROVIDER_TYPE,
    GEMINI_EMBEDDING_PROVIDER_TYPE, GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE,
    GENIE_TEXT_TO_SPEECH_PROVIDER_TYPE, GOOGLE_GENAI_CHAT_PROVIDER_TYPE, GROQ_CHAT_PROVIDER_TYPE,
    GSV_SELFHOST_TEXT_TO_SPEECH_PROVIDER_TYPE, GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE,
    MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE, MOCK_CHAT_PROVIDER_TYPE, MOCK_EMBEDDING_PROVIDER_TYPE,
    MOCK_RERANK_PROVIDER_TYPE, MOCK_SPEECH_TO_TEXT_PROVIDER_TYPE,
    MOCK_TEXT_TO_SPEECH_PROVIDER_TYPE, OPENAI_CHAT_PROVIDER_TYPE,
    OPENAI_COMPATIBLE_CHAT_PROVIDER_TYPES, OPENAI_EMBEDDING_PROVIDER_TYPE,
    OPENAI_SPEECH_TO_TEXT_PROVIDER_TYPE, OPENAI_TEXT_TO_SPEECH_PROVIDER_TYPE,
    OPENAI_WHISPER_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE, OPENROUTER_CHAT_PROVIDER_TYPE,
    SENSEVOICE_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE, VLLM_RERANK_PROVIDER_TYPE,
    VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE, XAI_CHAT_PROVIDER_TYPE,
    XINFERENCE_RERANK_PROVIDER_TYPE, XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE,
    ZHIPU_CHAT_PROVIDER_TYPE,
};
pub use embedding::{EmbeddingProvider, EmbeddingRequest, EmbeddingResponse};
pub use gemini::{GeminiConfig, GeminiProvider};
pub use gemini_embedding::{GeminiEmbeddingConfig, GeminiEmbeddingProvider};
pub use gemini_tts::{GeminiTextToSpeechConfig, GeminiTextToSpeechProvider};
pub use gsv_selfhost_tts::{GsvSelfhostTextToSpeechConfig, GsvSelfhostTextToSpeechProvider};
pub use gsvi_tts::{GsviTextToSpeechConfig, GsviTextToSpeechProvider};
pub use manager::{
    NoopProviderSelectionHook, ProviderManager, ProviderManagerConfigSet, ProviderSelectionChanged,
    ProviderSelectionHook,
};
pub use minimax_tts::{MiniMaxTextToSpeechConfig, MiniMaxTextToSpeechProvider};
pub use mock::{
    MockChatProvider, MockEmbeddingProvider, MockRerankProvider, MockSpeechToTextProvider,
    MockTextToSpeechProvider,
};
pub use model_discovery::{
    ProviderModelDiscoveryConfig, ProviderModelDiscoveryResult, default_model_candidates,
    discover_provider_models, model_discovery_support, model_metadata_map, provider_model_ids,
    sanitize_model_discovery_error,
};
pub use openai_compatible::{OpenAiCompatibleConfig, OpenAiCompatibleProvider};
pub use openai_embedding::{OpenAiEmbeddingConfig, OpenAiEmbeddingProvider};
pub use openai_stt::{OpenAiSpeechToTextConfig, OpenAiSpeechToTextProvider};
pub use openai_tts::{OpenAiTextToSpeechConfig, OpenAiTextToSpeechProvider};
pub use registry::ProviderRegistry;
pub use rerank::{RerankDocumentScore, RerankProvider, RerankRequest, RerankResponse};
pub use response::{
    ProviderRawResponse, ProviderReasoningMetadata, ProviderResponse, ProviderResponseMetadata,
    ProviderStreamEvent, ProviderStreamEventKind, ProviderTokenUsage, ProviderToolCall,
    ProviderToolCallArguments,
};
pub use selection::{ProviderSelection, ProviderSelectionScope, ProviderSelectionState};
pub use selfhost_stt::{
    SelfhostSpeechToTextConfig, SelfhostSpeechToTextKind, SelfhostSpeechToTextProvider,
};
pub use speech::{SpeechToTextProvider, SpeechToTextRequest, SpeechToTextResponse};
pub use tts::{
    FileSynthesisTextToSpeechStreamProvider, QueuedTextToSpeechAudioStream, TextToSpeechAudioChunk,
    TextToSpeechAudioQueueItem, TextToSpeechAudioStream, TextToSpeechProvider, TextToSpeechRequest,
    TextToSpeechResponse, TextToSpeechStreamProvider, TextToSpeechStreamRequest,
    TextToSpeechStreamText,
};
pub use vllm_rerank::{VllmRerankConfig, VllmRerankProvider};
pub use volcengine_tts::{VolcengineTextToSpeechConfig, VolcengineTextToSpeechProvider};
pub use xinference_rerank::{XinferenceRerankConfig, XinferenceRerankProvider};
pub use xinference_stt::{XinferenceSpeechToTextConfig, XinferenceSpeechToTextProvider};
