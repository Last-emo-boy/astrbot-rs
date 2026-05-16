use astrbot_provider::ProviderManager;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderManagementResponse {
    pub chat_provider_count: usize,
    pub default_chat_provider_id: Option<String>,
    pub speech_to_text_provider_count: usize,
    pub default_speech_to_text_provider_id: Option<String>,
    pub text_to_speech_provider_count: usize,
    pub default_text_to_speech_provider_id: Option<String>,
    pub supports_text_to_speech_streaming: bool,
    pub embedding_provider_count: usize,
    pub default_embedding_provider_id: Option<String>,
    pub rerank_provider_count: usize,
    pub default_rerank_provider_id: Option<String>,
}

impl ProviderManagementResponse {
    pub fn from_manager(manager: &ProviderManager) -> Self {
        Self {
            chat_provider_count: manager.chat_provider_count(),
            default_chat_provider_id: manager.default_chat_provider_id().map(str::to_string),
            speech_to_text_provider_count: manager.speech_to_text_provider_count(),
            default_speech_to_text_provider_id: manager
                .default_speech_to_text_provider_id()
                .map(str::to_string),
            text_to_speech_provider_count: manager.text_to_speech_provider_count(),
            default_text_to_speech_provider_id: manager
                .default_text_to_speech_provider_id()
                .map(str::to_string),
            supports_text_to_speech_streaming: manager.supports_text_to_speech_streaming(),
            embedding_provider_count: manager.embedding_provider_count(),
            default_embedding_provider_id: manager
                .default_embedding_provider_id()
                .map(str::to_string),
            rerank_provider_count: manager.rerank_provider_count(),
            default_rerank_provider_id: manager.default_rerank_provider_id().map(str::to_string),
        }
    }
}
