mod chat;
mod common;
mod embedding;
mod rerank;
mod speech;
mod tts;

pub(crate) use chat::{
    OpenAiCompatiblePreset, build_anthropic_provider, build_gemini_provider,
    build_openai_compatible_provider, register_openai_compatible_alias,
};
pub(crate) use embedding::{build_gemini_embedding_provider, build_openai_embedding_provider};
pub(crate) use rerank::{
    build_bailian_rerank_provider, build_vllm_rerank_provider, build_xinference_rerank_provider,
};
pub(crate) use speech::{
    build_openai_speech_to_text_provider, build_xinference_speech_to_text_provider,
};
pub(crate) use tts::{
    build_gemini_text_to_speech_provider, build_gsvi_text_to_speech_provider,
    build_minimax_text_to_speech_provider, build_openai_text_to_speech_provider,
    build_volcengine_text_to_speech_provider,
};
