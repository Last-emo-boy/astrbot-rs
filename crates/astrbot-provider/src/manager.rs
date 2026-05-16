use std::sync::Arc;

use astrbot_core::Result;
use astrbot_observability::{NoopStatusEventSink, StatusEventSink};

mod bucket;
mod chat;
mod config_set;
mod embedding;
mod hooks;
mod lifecycle;
mod rerank;
mod speech;
mod tts;

pub use config_set::ProviderManagerConfigSet;
pub use hooks::{NoopProviderSelectionHook, ProviderSelectionChanged, ProviderSelectionHook};

use crate::config::{
    ChatProviderConfig, EmbeddingProviderConfig, RerankProviderConfig, SpeechToTextProviderConfig,
    TextToSpeechProviderConfig,
};
use crate::registry::ProviderRegistry;
use crate::{
    ChatProvider, EmbeddingProvider, RerankProvider, SpeechToTextProvider, TextToSpeechProvider,
};
use crate::{ProviderCapability, ProviderSelection, ProviderSelectionState};
use bucket::ProviderBucket;

#[derive(Clone)]
pub struct ProviderManager {
    chat_providers: ProviderBucket<dyn ChatProvider>,
    speech_to_text_providers: ProviderBucket<dyn SpeechToTextProvider>,
    text_to_speech_providers: ProviderBucket<dyn TextToSpeechProvider>,
    embedding_providers: ProviderBucket<dyn EmbeddingProvider>,
    rerank_providers: ProviderBucket<dyn RerankProvider>,
    selection_state: ProviderSelectionState,
    selection_hook: Arc<dyn ProviderSelectionHook>,
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
        let chat_providers = ProviderBucket::from_configs(
            configs.chat_providers,
            configs.default_chat_provider_id,
            |config| registry.build_chat_provider(config),
        )?;
        let speech_to_text_providers = ProviderBucket::from_configs(
            configs.speech_to_text_providers,
            configs.default_speech_to_text_provider_id,
            |config| registry.build_speech_to_text_provider(config),
        )?;
        let text_to_speech_providers = ProviderBucket::from_configs(
            configs.text_to_speech_providers,
            configs.default_text_to_speech_provider_id,
            |config| registry.build_text_to_speech_provider(config),
        )?;
        let embedding_providers = ProviderBucket::from_configs(
            configs.embedding_providers,
            configs.default_embedding_provider_id,
            |config| registry.build_embedding_provider(config),
        )?;
        let rerank_providers = ProviderBucket::from_configs(
            configs.rerank_providers,
            configs.default_rerank_provider_id,
            |config| registry.build_rerank_provider(config),
        )?;

        let mut selection_state = ProviderSelectionState::new();
        set_default_selection(
            &mut selection_state,
            ProviderCapability::ChatCompletion,
            chat_providers.default_provider_id.clone(),
        );
        set_default_selection(
            &mut selection_state,
            ProviderCapability::SpeechToText,
            speech_to_text_providers.default_provider_id.clone(),
        );
        set_default_selection(
            &mut selection_state,
            ProviderCapability::TextToSpeech,
            text_to_speech_providers.default_provider_id.clone(),
        );
        set_default_selection(
            &mut selection_state,
            ProviderCapability::Embedding,
            embedding_providers.default_provider_id.clone(),
        );
        set_default_selection(
            &mut selection_state,
            ProviderCapability::Rerank,
            rerank_providers.default_provider_id.clone(),
        );

        Ok(Self {
            chat_providers: chat_providers.bucket,
            speech_to_text_providers: speech_to_text_providers.bucket,
            text_to_speech_providers: text_to_speech_providers.bucket,
            embedding_providers: embedding_providers.bucket,
            rerank_providers: rerank_providers.bucket,
            selection_state,
            selection_hook: Arc::new(NoopProviderSelectionHook),
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
            chat_providers: ProviderBucket::default(),
            speech_to_text_providers: ProviderBucket::default(),
            text_to_speech_providers: ProviderBucket::default(),
            embedding_providers: ProviderBucket::default(),
            rerank_providers: ProviderBucket::default(),
            selection_state: ProviderSelectionState::new(),
            selection_hook: Arc::new(NoopProviderSelectionHook),
            status_sink: Arc::new(NoopStatusEventSink),
        }
    }

    pub fn with_status_sink(mut self, status_sink: Arc<dyn StatusEventSink>) -> Self {
        self.status_sink = status_sink;
        self
    }

    pub fn with_selection_hook(mut self, selection_hook: Arc<dyn ProviderSelectionHook>) -> Self {
        self.selection_hook = selection_hook;
        self
    }

    pub fn selection_state(&self) -> &ProviderSelectionState {
        &self.selection_state
    }

    pub fn selected_provider_id(
        &self,
        capability: ProviderCapability,
        session_id: Option<&str>,
    ) -> Option<&str> {
        self.selection_state
            .selected_provider_id(capability, session_id)
    }

    pub fn set_default_provider_id(
        &mut self,
        capability: ProviderCapability,
        provider_id: impl Into<String>,
    ) -> Result<()> {
        let provider_id = provider_id.into();
        self.ensure_provider_exists(capability, &provider_id)?;
        let previous_provider_id = self
            .selection_state
            .set_default_provider_id(capability, provider_id.clone());
        self.selection_hook
            .provider_selection_changed(&ProviderSelectionChanged::new(
                ProviderSelection::default_provider(capability, provider_id),
                previous_provider_id,
            ));
        Ok(())
    }

    pub fn set_session_provider_id(
        &mut self,
        capability: ProviderCapability,
        session_id: impl Into<String>,
        provider_id: impl Into<String>,
    ) -> Result<()> {
        let session_id = session_id.into();
        let provider_id = provider_id.into();
        self.ensure_provider_exists(capability, &provider_id)?;
        let previous_provider_id = self.selection_state.set_session_provider_id(
            capability,
            session_id.clone(),
            provider_id.clone(),
        );
        self.selection_hook
            .provider_selection_changed(&ProviderSelectionChanged::new(
                ProviderSelection::session_provider(capability, session_id, provider_id),
                previous_provider_id,
            ));
        Ok(())
    }

    pub fn chat_provider(&self, id: &str) -> Option<Arc<dyn ChatProvider>> {
        self.chat_providers.get(id)
    }

    pub fn default_chat_provider(&self) -> Option<Arc<dyn ChatProvider>> {
        self.chat_providers
            .selected(self.default_chat_provider_id())
    }

    pub fn default_chat_provider_id(&self) -> Option<&str> {
        self.selected_provider_id(ProviderCapability::ChatCompletion, None)
    }

    pub fn chat_provider_count(&self) -> usize {
        self.chat_providers.len()
    }

    pub fn speech_to_text_provider(&self, id: &str) -> Option<Arc<dyn SpeechToTextProvider>> {
        self.speech_to_text_providers.get(id)
    }

    pub fn default_speech_to_text_provider(&self) -> Option<Arc<dyn SpeechToTextProvider>> {
        self.speech_to_text_providers
            .selected(self.default_speech_to_text_provider_id())
    }

    pub fn default_speech_to_text_provider_id(&self) -> Option<&str> {
        self.selected_provider_id(ProviderCapability::SpeechToText, None)
    }

    pub fn speech_to_text_provider_count(&self) -> usize {
        self.speech_to_text_providers.len()
    }

    pub fn text_to_speech_provider(&self, id: &str) -> Option<Arc<dyn TextToSpeechProvider>> {
        self.text_to_speech_providers.get(id)
    }

    pub fn default_text_to_speech_provider(&self) -> Option<Arc<dyn TextToSpeechProvider>> {
        self.text_to_speech_providers
            .selected(self.default_text_to_speech_provider_id())
    }

    pub fn default_text_to_speech_provider_id(&self) -> Option<&str> {
        self.selected_provider_id(ProviderCapability::TextToSpeech, None)
    }

    pub fn text_to_speech_provider_count(&self) -> usize {
        self.text_to_speech_providers.len()
    }

    pub fn supports_text_to_speech_streaming(&self) -> bool {
        self.default_text_to_speech_provider()
            .is_some_and(|provider| provider.supports_streaming())
    }

    pub fn embedding_provider(&self, id: &str) -> Option<Arc<dyn EmbeddingProvider>> {
        self.embedding_providers.get(id)
    }

    pub fn default_embedding_provider(&self) -> Option<Arc<dyn EmbeddingProvider>> {
        self.embedding_providers
            .selected(self.default_embedding_provider_id())
    }

    pub fn default_embedding_provider_id(&self) -> Option<&str> {
        self.selected_provider_id(ProviderCapability::Embedding, None)
    }

    pub fn embedding_provider_count(&self) -> usize {
        self.embedding_providers.len()
    }

    pub fn rerank_provider(&self, id: &str) -> Option<Arc<dyn RerankProvider>> {
        self.rerank_providers.get(id)
    }

    pub fn default_rerank_provider(&self) -> Option<Arc<dyn RerankProvider>> {
        self.rerank_providers
            .selected(self.default_rerank_provider_id())
    }

    pub fn default_rerank_provider_id(&self) -> Option<&str> {
        self.selected_provider_id(ProviderCapability::Rerank, None)
    }

    pub fn rerank_provider_count(&self) -> usize {
        self.rerank_providers.len()
    }

    fn ensure_provider_exists(
        &self,
        capability: ProviderCapability,
        provider_id: &str,
    ) -> Result<()> {
        let exists = match capability {
            ProviderCapability::ChatCompletion => self.chat_providers.contains_key(provider_id),
            ProviderCapability::SpeechToText => {
                self.speech_to_text_providers.contains_key(provider_id)
            }
            ProviderCapability::TextToSpeech => {
                self.text_to_speech_providers.contains_key(provider_id)
            }
            ProviderCapability::Embedding => self.embedding_providers.contains_key(provider_id),
            ProviderCapability::Rerank => self.rerank_providers.contains_key(provider_id),
        };

        if exists {
            Ok(())
        } else {
            Err(astrbot_core::AstrbotError::Provider(format!(
                "{capability} provider {provider_id} is not configured"
            )))
        }
    }
}

fn set_default_selection(
    selection_state: &mut ProviderSelectionState,
    capability: ProviderCapability,
    provider_id: Option<String>,
) {
    if let Some(provider_id) = provider_id {
        selection_state.set_default_provider_id(capability, provider_id);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::{ProviderManager, ProviderSelectionChanged, ProviderSelectionHook};
    use crate::{ChatProvider, ChatRequest, ChatResponse, ProviderCapability};

    struct TestChatProvider;

    #[async_trait]
    impl ChatProvider for TestChatProvider {
        async fn chat(&self, _request: ChatRequest) -> astrbot_core::Result<ChatResponse> {
            Ok(ChatResponse::text("ok"))
        }
    }

    #[derive(Default)]
    struct RecordingSelectionHook {
        events: Mutex<Vec<ProviderSelectionChanged>>,
    }

    impl ProviderSelectionHook for RecordingSelectionHook {
        fn provider_selection_changed(&self, event: &ProviderSelectionChanged) {
            self.events
                .lock()
                .expect("selection events lock")
                .push(event.clone());
        }
    }

    #[test]
    fn manager_updates_default_selection_through_hook() {
        let hook = Arc::new(RecordingSelectionHook::default());
        let mut manager = ProviderManager::empty().with_selection_hook(hook.clone());
        manager
            .chat_providers
            .insert("chat-a".to_string(), Arc::new(TestChatProvider));

        manager
            .set_default_provider_id(ProviderCapability::ChatCompletion, "chat-a")
            .expect("configured provider should be selectable");

        assert_eq!(manager.default_chat_provider_id(), Some("chat-a"));
        let events = hook.events.lock().expect("selection events lock");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].selection.provider_id, "chat-a");
        assert_eq!(
            events[0].selection.capability,
            ProviderCapability::ChatCompletion
        );
    }

    #[test]
    fn manager_rejects_selection_for_unconfigured_capability_provider() {
        let mut manager = ProviderManager::empty();
        manager
            .chat_providers
            .insert("chat-a".to_string(), Arc::new(TestChatProvider));

        let err = manager
            .set_default_provider_id(ProviderCapability::Embedding, "chat-a")
            .expect_err("provider id must exist for the selected capability");

        assert!(err.to_string().contains("embedding provider chat-a"));
    }
}
