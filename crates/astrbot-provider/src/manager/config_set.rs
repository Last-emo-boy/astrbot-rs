use crate::config::{
    ChatProviderConfig, EmbeddingProviderConfig, RerankProviderConfig, SpeechToTextProviderConfig,
    TextToSpeechProviderConfig,
};

#[derive(Clone, Debug, Default)]
pub struct ProviderManagerConfigSet {
    pub chat_providers: Vec<ChatProviderConfig>,
    pub default_chat_provider_id: Option<String>,
    pub speech_to_text_providers: Vec<SpeechToTextProviderConfig>,
    pub default_speech_to_text_provider_id: Option<String>,
    pub text_to_speech_providers: Vec<TextToSpeechProviderConfig>,
    pub default_text_to_speech_provider_id: Option<String>,
    pub embedding_providers: Vec<EmbeddingProviderConfig>,
    pub default_embedding_provider_id: Option<String>,
    pub rerank_providers: Vec<RerankProviderConfig>,
    pub default_rerank_provider_id: Option<String>,
}
