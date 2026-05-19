use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
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
    pub model_discovery: ProviderModelDiscoverySupport,
    pub model_candidates: Vec<ProviderModelInfo>,
}

impl ProviderAdapterMetadata {
    pub fn new(provider_type: impl Into<String>, capability: ProviderCapability) -> Self {
        Self {
            provider_type: provider_type.into(),
            capability,
            model_discovery: ProviderModelDiscoverySupport::Unsupported,
            model_candidates: Vec::new(),
        }
    }

    pub fn with_model_discovery(mut self, support: ProviderModelDiscoverySupport) -> Self {
        self.model_discovery = support;
        self
    }

    pub fn with_model_candidates(mut self, candidates: Vec<ProviderModelInfo>) -> Self {
        self.model_candidates = candidates;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderModelDiscoverySupport {
    Supported,
    Unsupported,
}

impl ProviderModelDiscoverySupport {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
        }
    }
}

impl fmt::Display for ProviderModelDiscoverySupport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderModelInfo {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

impl ProviderModelInfo {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: None,
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}
