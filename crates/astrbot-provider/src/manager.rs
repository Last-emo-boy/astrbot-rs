use std::collections::HashMap;
use std::sync::Arc;

use astrbot_core::Result;
use astrbot_observability::{NoopStatusEventSink, StatusEventSink};

mod chat;
mod config_set;
mod embedding;
mod lifecycle;
mod rerank;
mod speech;
mod tts;

pub use config_set::ProviderManagerConfigSet;

use crate::config::{
    ChatProviderConfig, EmbeddingProviderConfig, RerankProviderConfig, SpeechToTextProviderConfig,
    TextToSpeechProviderConfig,
};
use crate::registry::ProviderRegistry;
use crate::{
    ChatProvider, EmbeddingProvider, RerankProvider, SpeechToTextProvider, TextToSpeechProvider,
};

#[derive(Clone)]
pub struct ProviderManager {
    pub(super) chat_providers: HashMap<String, Arc<dyn ChatProvider>>,
    pub(super) default_chat_provider_id: Option<String>,
    pub(super) speech_to_text_providers: HashMap<String, Arc<dyn SpeechToTextProvider>>,
    pub(super) default_speech_to_text_provider_id: Option<String>,
    pub(super) text_to_speech_providers: HashMap<String, Arc<dyn TextToSpeechProvider>>,
    pub(super) default_text_to_speech_provider_id: Option<String>,
    pub(super) embedding_providers: HashMap<String, Arc<dyn EmbeddingProvider>>,
    pub(super) default_embedding_provider_id: Option<String>,
    pub(super) rerank_providers: HashMap<String, Arc<dyn RerankProvider>>,
    pub(super) default_rerank_provider_id: Option<String>,
    pub(super) status_sink: Arc<dyn StatusEventSink>,
}

impl Default for ProviderManager {
    fn default() -> Self {
        Self::empty()
    }
}

impl ProviderManager {
    pub fn from_configs(
        registry: &ProviderRegistry,
        configs: ProviderManagerConfigSet,
    ) -> Result<Self> {
        let mut chat_providers = HashMap::new();
        let mut first_chat_provider_id = None;
        for config in configs.chat_providers {
            if !config.enabled {
                continue;
            }

            let provider = registry.build_chat_provider(&config)?;
            first_chat_provider_id.get_or_insert_with(|| config.id.clone());
            chat_providers.insert(config.id, provider);
        }
        let default_chat_provider_id = configs
            .default_chat_provider_id
            .filter(|id| chat_providers.contains_key(id))
            .or(first_chat_provider_id);

        let mut speech_to_text_providers = HashMap::new();
        let mut first_speech_to_text_provider_id = None;
        for config in configs.speech_to_text_providers {
            if !config.enabled {
                continue;
            }

            let provider = registry.build_speech_to_text_provider(&config)?;
            first_speech_to_text_provider_id.get_or_insert_with(|| config.id.clone());
            speech_to_text_providers.insert(config.id, provider);
        }
        let default_speech_to_text_provider_id = configs
            .default_speech_to_text_provider_id
            .filter(|id| speech_to_text_providers.contains_key(id))
            .or(first_speech_to_text_provider_id);

        let mut text_to_speech_providers = HashMap::new();
        let mut first_text_to_speech_provider_id = None;
        for config in configs.text_to_speech_providers {
            if !config.enabled {
                continue;
            }

            let provider = registry.build_text_to_speech_provider(&config)?;
            first_text_to_speech_provider_id.get_or_insert_with(|| config.id.clone());
            text_to_speech_providers.insert(config.id, provider);
        }
        let default_text_to_speech_provider_id = configs
            .default_text_to_speech_provider_id
            .filter(|id| text_to_speech_providers.contains_key(id))
            .or(first_text_to_speech_provider_id);

        let mut embedding_providers = HashMap::new();
        let mut first_embedding_provider_id = None;
        for config in configs.embedding_providers {
            if !config.enabled {
                continue;
            }

            let provider = registry.build_embedding_provider(&config)?;
            first_embedding_provider_id.get_or_insert_with(|| config.id.clone());
            embedding_providers.insert(config.id, provider);
        }
        let default_embedding_provider_id = configs
            .default_embedding_provider_id
            .filter(|id| embedding_providers.contains_key(id))
            .or(first_embedding_provider_id);

        let mut rerank_providers = HashMap::new();
        let mut first_rerank_provider_id = None;
        for config in configs.rerank_providers {
            if !config.enabled {
                continue;
            }

            let provider = registry.build_rerank_provider(&config)?;
            first_rerank_provider_id.get_or_insert_with(|| config.id.clone());
            rerank_providers.insert(config.id, provider);
        }
        let default_rerank_provider_id = configs
            .default_rerank_provider_id
            .filter(|id| rerank_providers.contains_key(id))
            .or(first_rerank_provider_id);

        Ok(Self {
            chat_providers,
            default_chat_provider_id,
            speech_to_text_providers,
            default_speech_to_text_provider_id,
            text_to_speech_providers,
            default_text_to_speech_provider_id,
            embedding_providers,
            default_embedding_provider_id,
            rerank_providers,
            default_rerank_provider_id,
            status_sink: Arc::new(NoopStatusEventSink),
        })
    }

    pub fn from_chat_configs(
        registry: &ProviderRegistry,
        configs: impl IntoIterator<Item = ChatProviderConfig>,
        default_chat_provider_id: Option<String>,
    ) -> Result<Self> {
        Self::from_configs(
            registry,
            ProviderManagerConfigSet {
                chat_providers: configs.into_iter().collect(),
                default_chat_provider_id,
                ..ProviderManagerConfigSet::default()
            },
        )
    }

    pub fn from_speech_to_text_configs(
        registry: &ProviderRegistry,
        configs: impl IntoIterator<Item = SpeechToTextProviderConfig>,
        default_speech_to_text_provider_id: Option<String>,
    ) -> Result<Self> {
        Self::from_configs(
            registry,
            ProviderManagerConfigSet {
                speech_to_text_providers: configs.into_iter().collect(),
                default_speech_to_text_provider_id,
                ..ProviderManagerConfigSet::default()
            },
        )
    }

    pub fn from_text_to_speech_configs(
        registry: &ProviderRegistry,
        configs: impl IntoIterator<Item = TextToSpeechProviderConfig>,
        default_text_to_speech_provider_id: Option<String>,
    ) -> Result<Self> {
        Self::from_configs(
            registry,
            ProviderManagerConfigSet {
                text_to_speech_providers: configs.into_iter().collect(),
                default_text_to_speech_provider_id,
                ..ProviderManagerConfigSet::default()
            },
        )
    }

    pub fn from_embedding_configs(
        registry: &ProviderRegistry,
        configs: impl IntoIterator<Item = EmbeddingProviderConfig>,
        default_embedding_provider_id: Option<String>,
    ) -> Result<Self> {
        Self::from_configs(
            registry,
            ProviderManagerConfigSet {
                embedding_providers: configs.into_iter().collect(),
                default_embedding_provider_id,
                ..ProviderManagerConfigSet::default()
            },
        )
    }

    pub fn from_rerank_configs(
        registry: &ProviderRegistry,
        configs: impl IntoIterator<Item = RerankProviderConfig>,
        default_rerank_provider_id: Option<String>,
    ) -> Result<Self> {
        Self::from_configs(
            registry,
            ProviderManagerConfigSet {
                rerank_providers: configs.into_iter().collect(),
                default_rerank_provider_id,
                ..ProviderManagerConfigSet::default()
            },
        )
    }

    pub fn empty() -> Self {
        Self {
            chat_providers: HashMap::new(),
            default_chat_provider_id: None,
            speech_to_text_providers: HashMap::new(),
            default_speech_to_text_provider_id: None,
            text_to_speech_providers: HashMap::new(),
            default_text_to_speech_provider_id: None,
            embedding_providers: HashMap::new(),
            default_embedding_provider_id: None,
            rerank_providers: HashMap::new(),
            default_rerank_provider_id: None,
            status_sink: Arc::new(NoopStatusEventSink),
        }
    }

    pub fn with_status_sink(mut self, status_sink: Arc<dyn StatusEventSink>) -> Self {
        self.status_sink = status_sink;
        self
    }

    pub fn chat_provider(&self, id: &str) -> Option<Arc<dyn ChatProvider>> {
        self.chat_providers.get(id).cloned()
    }

    pub fn default_chat_provider(&self) -> Option<Arc<dyn ChatProvider>> {
        self.default_chat_provider_id
            .as_deref()
            .and_then(|id| self.chat_provider(id))
    }

    pub fn default_chat_provider_id(&self) -> Option<&str> {
        self.default_chat_provider_id.as_deref()
    }

    pub fn chat_provider_count(&self) -> usize {
        self.chat_providers.len()
    }

    pub fn speech_to_text_provider(&self, id: &str) -> Option<Arc<dyn SpeechToTextProvider>> {
        self.speech_to_text_providers.get(id).cloned()
    }

    pub fn default_speech_to_text_provider(&self) -> Option<Arc<dyn SpeechToTextProvider>> {
        self.default_speech_to_text_provider_id
            .as_deref()
            .and_then(|id| self.speech_to_text_provider(id))
    }

    pub fn default_speech_to_text_provider_id(&self) -> Option<&str> {
        self.default_speech_to_text_provider_id.as_deref()
    }

    pub fn speech_to_text_provider_count(&self) -> usize {
        self.speech_to_text_providers.len()
    }

    pub fn text_to_speech_provider(&self, id: &str) -> Option<Arc<dyn TextToSpeechProvider>> {
        self.text_to_speech_providers.get(id).cloned()
    }

    pub fn default_text_to_speech_provider(&self) -> Option<Arc<dyn TextToSpeechProvider>> {
        self.default_text_to_speech_provider_id
            .as_deref()
            .and_then(|id| self.text_to_speech_provider(id))
    }

    pub fn default_text_to_speech_provider_id(&self) -> Option<&str> {
        self.default_text_to_speech_provider_id.as_deref()
    }

    pub fn text_to_speech_provider_count(&self) -> usize {
        self.text_to_speech_providers.len()
    }

    pub fn supports_text_to_speech_streaming(&self) -> bool {
        self.default_text_to_speech_provider()
            .is_some_and(|provider| provider.supports_streaming())
    }

    pub fn embedding_provider(&self, id: &str) -> Option<Arc<dyn EmbeddingProvider>> {
        self.embedding_providers.get(id).cloned()
    }

    pub fn default_embedding_provider(&self) -> Option<Arc<dyn EmbeddingProvider>> {
        self.default_embedding_provider_id
            .as_deref()
            .and_then(|id| self.embedding_provider(id))
    }

    pub fn default_embedding_provider_id(&self) -> Option<&str> {
        self.default_embedding_provider_id.as_deref()
    }

    pub fn embedding_provider_count(&self) -> usize {
        self.embedding_providers.len()
    }

    pub fn rerank_provider(&self, id: &str) -> Option<Arc<dyn RerankProvider>> {
        self.rerank_providers.get(id).cloned()
    }

    pub fn default_rerank_provider(&self) -> Option<Arc<dyn RerankProvider>> {
        self.default_rerank_provider_id
            .as_deref()
            .and_then(|id| self.rerank_provider(id))
    }

    pub fn default_rerank_provider_id(&self) -> Option<&str> {
        self.default_rerank_provider_id.as_deref()
    }

    pub fn rerank_provider_count(&self) -> usize {
        self.rerank_providers.len()
    }
}
