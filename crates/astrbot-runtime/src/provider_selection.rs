use astrbot_provider::{
    ChatProviderConfig, EmbeddingProviderConfig, ProviderCapability, ProviderManagerConfigSet,
    ProviderSelection, ProviderSelectionState, RerankProviderConfig, SpeechToTextProviderConfig,
    TextToSpeechProviderConfig,
};

use crate::RuntimeConfig;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeProviderSelectionSnapshot {
    selections: Vec<ProviderSelection>,
}

impl RuntimeProviderSelectionSnapshot {
    pub fn from_config(config: &RuntimeConfig) -> Self {
        let mut state = ProviderSelectionState::new();
        state.set_default_provider_id(
            ProviderCapability::ChatCompletion,
            config.default_chat_provider_id.clone(),
        );
        set_optional_default(
            &mut state,
            ProviderCapability::SpeechToText,
            config.default_speech_to_text_provider_id.clone(),
        );
        set_optional_default(
            &mut state,
            ProviderCapability::TextToSpeech,
            config.default_text_to_speech_provider_id.clone(),
        );
        set_optional_default(
            &mut state,
            ProviderCapability::Embedding,
            config.default_embedding_provider_id.clone(),
        );
        set_optional_default(
            &mut state,
            ProviderCapability::Rerank,
            config.default_rerank_provider_id.clone(),
        );

        Self {
            selections: state.default_selections(),
        }
    }

    pub fn selections(&self) -> &[ProviderSelection] {
        &self.selections
    }
}

fn set_optional_default(
    state: &mut ProviderSelectionState,
    capability: ProviderCapability,
    provider_id: Option<String>,
) {
    if let Some(provider_id) = provider_id.filter(|provider_id| !provider_id.trim().is_empty()) {
        state.set_default_provider_id(capability, provider_id);
    }
}

pub(crate) fn provider_manager_config_set(config: &RuntimeConfig) -> ProviderManagerConfigSet {
    ProviderManagerConfigSet {
        chat_providers: config
            .chat_providers
            .clone()
            .into_iter()
            .map(ChatProviderConfig::from)
            .collect(),
        default_chat_provider_id: Some(config.default_chat_provider_id.clone()),
        speech_to_text_providers: config
            .speech_to_text_providers
            .clone()
            .into_iter()
            .map(SpeechToTextProviderConfig::from)
            .collect(),
        default_speech_to_text_provider_id: config.default_speech_to_text_provider_id.clone(),
        text_to_speech_providers: config
            .text_to_speech_providers
            .clone()
            .into_iter()
            .map(TextToSpeechProviderConfig::from)
            .collect(),
        default_text_to_speech_provider_id: config.default_text_to_speech_provider_id.clone(),
        embedding_providers: config
            .embedding_providers
            .clone()
            .into_iter()
            .map(EmbeddingProviderConfig::from)
            .collect(),
        default_embedding_provider_id: config.default_embedding_provider_id.clone(),
        rerank_providers: config
            .rerank_providers
            .clone()
            .into_iter()
            .map(RerankProviderConfig::from)
            .collect(),
        default_rerank_provider_id: config.default_rerank_provider_id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use astrbot_provider::ProviderCapability;

    use super::RuntimeProviderSelectionSnapshot;
    use crate::RuntimeConfig;

    #[test]
    fn runtime_selection_snapshot_reads_configured_defaults_by_capability() {
        let config = RuntimeConfig {
            default_chat_provider_id: "chat-a".to_string(),
            default_speech_to_text_provider_id: Some("stt-a".to_string()),
            default_text_to_speech_provider_id: Some("tts-a".to_string()),
            default_embedding_provider_id: Some("embedding-a".to_string()),
            default_rerank_provider_id: Some("rerank-a".to_string()),
            ..RuntimeConfig::default()
        };

        let snapshot = RuntimeProviderSelectionSnapshot::from_config(&config);
        let selections = snapshot.selections();

        assert_eq!(selections.len(), 5);
        assert!(selections.iter().any(|selection| {
            selection.capability == ProviderCapability::ChatCompletion
                && selection.provider_id == "chat-a"
        }));
        assert!(selections.iter().any(|selection| selection.capability
            == ProviderCapability::Rerank
            && selection.provider_id == "rerank-a"));
    }

    #[test]
    fn runtime_selection_snapshot_omits_blank_optional_defaults() {
        let config = RuntimeConfig {
            default_chat_provider_id: "chat-a".to_string(),
            default_embedding_provider_id: Some(" ".to_string()),
            ..RuntimeConfig::default()
        };

        let snapshot = RuntimeProviderSelectionSnapshot::from_config(&config);

        assert_eq!(snapshot.selections().len(), 1);
        assert_eq!(snapshot.selections()[0].provider_id, "chat-a");
    }
}
