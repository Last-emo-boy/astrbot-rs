use std::collections::HashMap;
use std::sync::Arc;

use astrbot_core::Result;

use crate::capability::{ProviderAdapterMetadata, ProviderCapability};
use crate::config::{
    ChatProviderConfig, EmbeddingProviderConfig, RerankProviderConfig, SpeechToTextProviderConfig,
    TextToSpeechProviderConfig,
};
use crate::{
    ChatProvider, EmbeddingProvider, RerankProvider, SpeechToTextProvider, TextToSpeechProvider,
};

mod builtins;
mod errors;
mod factory;
mod metadata;

use errors::missing_factory_error;
use factory::{
    ChatProviderFactory, EmbeddingProviderFactory, RerankProviderFactory,
    SpeechToTextProviderFactory, TextToSpeechProviderFactory,
};
use metadata::ProviderMetadataIndex;

#[derive(Clone, Default)]
pub struct ProviderRegistry {
    adapter_metadata: ProviderMetadataIndex,
    chat_factories: HashMap<String, ChatProviderFactory>,
    speech_to_text_factories: HashMap<String, SpeechToTextProviderFactory>,
    text_to_speech_factories: HashMap<String, TextToSpeechProviderFactory>,
    embedding_factories: HashMap<String, EmbeddingProviderFactory>,
    rerank_factories: HashMap<String, RerankProviderFactory>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtin_providers() -> Self {
        builtins::with_builtin_providers()
    }

    pub fn with_builtin_chat_providers() -> Self {
        builtins::with_builtin_chat_providers()
    }

    pub fn register_chat_provider(
        &mut self,
        provider_type: impl Into<String>,
        factory: impl Fn(&ChatProviderConfig) -> Result<Arc<dyn ChatProvider>> + Send + Sync + 'static,
    ) -> Result<()> {
        let provider_type = provider_type.into();
        self.register_provider_adapter(&provider_type, ProviderCapability::ChatCompletion)?;
        self.chat_factories.insert(provider_type, Arc::new(factory));
        Ok(())
    }

    pub fn register_speech_to_text_provider(
        &mut self,
        provider_type: impl Into<String>,
        factory: impl Fn(&SpeechToTextProviderConfig) -> Result<Arc<dyn SpeechToTextProvider>>
        + Send
        + Sync
        + 'static,
    ) -> Result<()> {
        let provider_type = provider_type.into();
        self.register_provider_adapter(&provider_type, ProviderCapability::SpeechToText)?;
        self.speech_to_text_factories
            .insert(provider_type, Arc::new(factory));
        Ok(())
    }

    pub fn register_text_to_speech_provider(
        &mut self,
        provider_type: impl Into<String>,
        factory: impl Fn(&TextToSpeechProviderConfig) -> Result<Arc<dyn TextToSpeechProvider>>
        + Send
        + Sync
        + 'static,
    ) -> Result<()> {
        let provider_type = provider_type.into();
        self.register_provider_adapter(&provider_type, ProviderCapability::TextToSpeech)?;
        self.text_to_speech_factories
            .insert(provider_type, Arc::new(factory));
        Ok(())
    }

    pub fn register_embedding_provider(
        &mut self,
        provider_type: impl Into<String>,
        factory: impl Fn(&EmbeddingProviderConfig) -> Result<Arc<dyn EmbeddingProvider>>
        + Send
        + Sync
        + 'static,
    ) -> Result<()> {
        let provider_type = provider_type.into();
        self.register_provider_adapter(&provider_type, ProviderCapability::Embedding)?;
        self.embedding_factories
            .insert(provider_type, Arc::new(factory));
        Ok(())
    }

    pub fn register_rerank_provider(
        &mut self,
        provider_type: impl Into<String>,
        factory: impl Fn(&RerankProviderConfig) -> Result<Arc<dyn RerankProvider>>
        + Send
        + Sync
        + 'static,
    ) -> Result<()> {
        let provider_type = provider_type.into();
        self.register_provider_adapter(&provider_type, ProviderCapability::Rerank)?;
        self.rerank_factories
            .insert(provider_type, Arc::new(factory));
        Ok(())
    }

    pub fn register_provider_adapter(
        &mut self,
        provider_type: impl Into<String>,
        capability: ProviderCapability,
    ) -> Result<()> {
        self.adapter_metadata.register(provider_type, capability)
    }

    pub fn provider_metadata(&self, provider_type: &str) -> Option<&ProviderAdapterMetadata> {
        self.adapter_metadata.get(provider_type)
    }

    pub fn has_provider_adapter(&self, provider_type: &str) -> bool {
        self.adapter_metadata.contains(provider_type)
    }

    pub fn has_chat_provider(&self, provider_type: &str) -> bool {
        self.chat_factories.contains_key(provider_type)
    }

    pub fn has_speech_to_text_provider(&self, provider_type: &str) -> bool {
        self.speech_to_text_factories.contains_key(provider_type)
    }

    pub fn has_text_to_speech_provider(&self, provider_type: &str) -> bool {
        self.text_to_speech_factories.contains_key(provider_type)
    }

    pub fn has_embedding_provider(&self, provider_type: &str) -> bool {
        self.embedding_factories.contains_key(provider_type)
    }

    pub fn has_rerank_provider(&self, provider_type: &str) -> bool {
        self.rerank_factories.contains_key(provider_type)
    }

    pub fn provider_types_by_capability(&self, capability: ProviderCapability) -> Vec<String> {
        self.adapter_metadata.types_by_capability(capability)
    }

    pub fn build_chat_provider(
        &self,
        config: &ChatProviderConfig,
    ) -> Result<Arc<dyn ChatProvider>> {
        let factory = self
            .chat_factories
            .get(&config.provider_type)
            .ok_or_else(|| {
                missing_factory_error(
                    &config.provider_type,
                    "chat provider",
                    "chat provider type",
                    self.adapter_metadata.get(&config.provider_type),
                )
            })?;
        factory(config)
    }

    pub fn build_speech_to_text_provider(
        &self,
        config: &SpeechToTextProviderConfig,
    ) -> Result<Arc<dyn SpeechToTextProvider>> {
        let factory = self
            .speech_to_text_factories
            .get(&config.provider_type)
            .ok_or_else(|| {
                missing_factory_error(
                    &config.provider_type,
                    "speech-to-text provider",
                    "speech-to-text provider type",
                    self.adapter_metadata.get(&config.provider_type),
                )
            })?;
        factory(config)
    }

    pub fn build_text_to_speech_provider(
        &self,
        config: &TextToSpeechProviderConfig,
    ) -> Result<Arc<dyn TextToSpeechProvider>> {
        let factory = self
            .text_to_speech_factories
            .get(&config.provider_type)
            .ok_or_else(|| {
                missing_factory_error(
                    &config.provider_type,
                    "text-to-speech provider",
                    "text-to-speech provider type",
                    self.adapter_metadata.get(&config.provider_type),
                )
            })?;
        factory(config)
    }

    pub fn build_embedding_provider(
        &self,
        config: &EmbeddingProviderConfig,
    ) -> Result<Arc<dyn EmbeddingProvider>> {
        let factory = self
            .embedding_factories
            .get(&config.provider_type)
            .ok_or_else(|| {
                missing_factory_error(
                    &config.provider_type,
                    "embedding provider",
                    "embedding provider type",
                    self.adapter_metadata.get(&config.provider_type),
                )
            })?;
        factory(config)
    }

    pub fn build_rerank_provider(
        &self,
        config: &RerankProviderConfig,
    ) -> Result<Arc<dyn RerankProvider>> {
        let factory = self
            .rerank_factories
            .get(&config.provider_type)
            .ok_or_else(|| {
                missing_factory_error(
                    &config.provider_type,
                    "rerank provider",
                    "rerank provider type",
                    self.adapter_metadata.get(&config.provider_type),
                )
            })?;
        factory(config)
    }
}
