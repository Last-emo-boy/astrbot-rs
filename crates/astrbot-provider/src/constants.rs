pub const MOCK_CHAT_PROVIDER_TYPE: &str = "mock_chat_completion";
pub const MOCK_SPEECH_TO_TEXT_PROVIDER_TYPE: &str = "mock_speech_to_text";
pub const OPENAI_SPEECH_TO_TEXT_PROVIDER_TYPE: &str = "openai_whisper_api";
pub const XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE: &str = "xinference_stt";
pub const OPENAI_WHISPER_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE: &str = "openai_whisper_selfhost";
pub const SENSEVOICE_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE: &str = "sensevoice_stt_selfhost";
pub const MOCK_TEXT_TO_SPEECH_PROVIDER_TYPE: &str = "mock_text_to_speech";
pub const OPENAI_TEXT_TO_SPEECH_PROVIDER_TYPE: &str = "openai_tts_api";
pub const GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE: &str = "gemini_tts";
pub const VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE: &str = "volcengine_tts";
pub const MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE: &str = "minimax_tts_api";
pub const GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE: &str = "gsvi_tts_api";
pub const GSV_SELFHOST_TEXT_TO_SPEECH_PROVIDER_TYPE: &str = "gsv_tts_selfhost";
pub const AZURE_TEXT_TO_SPEECH_PROVIDER_TYPE: &str = "azure_tts";
pub const EDGE_TEXT_TO_SPEECH_PROVIDER_TYPE: &str = "edge_tts";
pub const DASHSCOPE_TEXT_TO_SPEECH_PROVIDER_TYPE: &str = "dashscope_tts";
pub const FISHAUDIO_TEXT_TO_SPEECH_PROVIDER_TYPE: &str = "fishaudio_tts_api";
pub const GENIE_TEXT_TO_SPEECH_PROVIDER_TYPE: &str = "genie_tts";
pub const MOCK_EMBEDDING_PROVIDER_TYPE: &str = "mock_embedding";
pub const MOCK_RERANK_PROVIDER_TYPE: &str = "mock_rerank";
pub const VLLM_RERANK_PROVIDER_TYPE: &str = "vllm_rerank";
pub const BAILIAN_RERANK_PROVIDER_TYPE: &str = "bailian_rerank";
pub const XINFERENCE_RERANK_PROVIDER_TYPE: &str = "xinference_rerank";
pub const OPENAI_EMBEDDING_PROVIDER_TYPE: &str = "openai_embedding";
pub const GEMINI_EMBEDDING_PROVIDER_TYPE: &str = "gemini_embedding";
pub const OPENAI_CHAT_PROVIDER_TYPE: &str = "openai_chat_completion";
pub const ANTHROPIC_CHAT_PROVIDER_TYPE: &str = "anthropic_chat_completion";
pub const GOOGLE_GENAI_CHAT_PROVIDER_TYPE: &str = "googlegenai_chat_completion";
pub const ZHIPU_CHAT_PROVIDER_TYPE: &str = "zhipu_chat_completion";
pub const GROQ_CHAT_PROVIDER_TYPE: &str = "groq_chat_completion";
pub const XAI_CHAT_PROVIDER_TYPE: &str = "xai_chat_completion";
pub const AIHUBMIX_CHAT_PROVIDER_TYPE: &str = "aihubmix_chat_completion";
pub const OPENROUTER_CHAT_PROVIDER_TYPE: &str = "openrouter_chat_completion";
pub const OPENAI_COMPATIBLE_CHAT_PROVIDER_TYPES: &[&str] = &[
    OPENAI_CHAT_PROVIDER_TYPE,
    ZHIPU_CHAT_PROVIDER_TYPE,
    GROQ_CHAT_PROVIDER_TYPE,
    XAI_CHAT_PROVIDER_TYPE,
    AIHUBMIX_CHAT_PROVIDER_TYPE,
    OPENROUTER_CHAT_PROVIDER_TYPE,
];
