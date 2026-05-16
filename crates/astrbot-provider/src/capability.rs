use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProviderCapability {
    ChatCompletion,
    SpeechToText,
    TextToSpeech,
    Embedding,
    Rerank,
}

impl ProviderCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChatCompletion => "chat_completion",
            Self::SpeechToText => "speech_to_text",
            Self::TextToSpeech => "text_to_speech",
            Self::Embedding => "embedding",
            Self::Rerank => "rerank",
        }
    }
}

impl fmt::Display for ProviderCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderAdapterMetadata {
    pub provider_type: String,
    pub capability: ProviderCapability,
}

impl ProviderAdapterMetadata {
    pub fn new(provider_type: impl Into<String>, capability: ProviderCapability) -> Self {
        Self {
            provider_type: provider_type.into(),
            capability,
        }
    }
}
