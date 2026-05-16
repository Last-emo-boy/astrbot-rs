use std::collections::HashMap;

use crate::ProviderCapability;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderSelectionScope {
    Default,
    Session { session_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderSelection {
    pub capability: ProviderCapability,
    pub provider_id: String,
    pub scope: ProviderSelectionScope,
}

impl ProviderSelection {
    pub fn default_provider(
        capability: ProviderCapability,
        provider_id: impl Into<String>,
    ) -> Self {
        Self {
            capability,
            provider_id: provider_id.into(),
            scope: ProviderSelectionScope::Default,
        }
    }

    pub fn session_provider(
        capability: ProviderCapability,
        session_id: impl Into<String>,
        provider_id: impl Into<String>,
    ) -> Self {
        Self {
            capability,
            provider_id: provider_id.into(),
            scope: ProviderSelectionScope::Session {
                session_id: session_id.into(),
            },
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderSelectionState {
    defaults: HashMap<ProviderCapability, String>,
    session_overrides: HashMap<ProviderSessionSelectionKey, String>,
}

impl ProviderSelectionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_provider_id(&self, capability: ProviderCapability) -> Option<&str> {
        self.defaults.get(&capability).map(String::as_str)
    }

    pub fn set_default_provider_id(
        &mut self,
        capability: ProviderCapability,
        provider_id: impl Into<String>,
    ) -> Option<String> {
        self.defaults.insert(capability, provider_id.into())
    }

    pub fn session_provider_id(
        &self,
        capability: ProviderCapability,
        session_id: &str,
    ) -> Option<&str> {
        self.session_overrides
            .get(&ProviderSessionSelectionKey::new(capability, session_id))
            .map(String::as_str)
    }

    pub fn set_session_provider_id(
        &mut self,
        capability: ProviderCapability,
        session_id: impl Into<String>,
        provider_id: impl Into<String>,
    ) -> Option<String> {
        self.session_overrides.insert(
            ProviderSessionSelectionKey::new(capability, session_id),
            provider_id.into(),
        )
    }

    pub fn selected_provider_id(
        &self,
        capability: ProviderCapability,
        session_id: Option<&str>,
    ) -> Option<&str> {
        session_id
            .and_then(|session_id| self.session_provider_id(capability, session_id))
            .or_else(|| self.default_provider_id(capability))
    }

    pub fn default_selections(&self) -> Vec<ProviderSelection> {
        let mut selections = self
            .defaults
            .iter()
            .map(|(capability, provider_id)| {
                ProviderSelection::default_provider(*capability, provider_id.clone())
            })
            .collect::<Vec<_>>();
        selections.sort_by_key(|selection| selection.capability.as_str());
        selections
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ProviderSessionSelectionKey {
    capability: ProviderCapability,
    session_id: String,
}

impl ProviderSessionSelectionKey {
    fn new(capability: ProviderCapability, session_id: impl Into<String>) -> Self {
        Self {
            capability,
            session_id: session_id.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{ProviderCapability, ProviderSelectionState};

    #[test]
    fn selection_state_falls_back_from_session_to_default_provider() {
        let mut state = ProviderSelectionState::new();
        state.set_default_provider_id(ProviderCapability::ChatCompletion, "default-chat");
        state.set_session_provider_id(
            ProviderCapability::ChatCompletion,
            "session-1",
            "session-chat",
        );

        assert_eq!(
            state.selected_provider_id(ProviderCapability::ChatCompletion, Some("session-1")),
            Some("session-chat")
        );
        assert_eq!(
            state.selected_provider_id(ProviderCapability::ChatCompletion, Some("session-2")),
            Some("default-chat")
        );
        assert_eq!(
            state.selected_provider_id(ProviderCapability::Embedding, Some("session-1")),
            None
        );
    }

    #[test]
    fn default_selections_are_stable_by_capability_name() {
        let mut state = ProviderSelectionState::new();
        state.set_default_provider_id(ProviderCapability::Rerank, "rerank");
        state.set_default_provider_id(ProviderCapability::ChatCompletion, "chat");

        let selections = state.default_selections();

        assert_eq!(selections[0].provider_id, "chat");
        assert_eq!(selections[1].provider_id, "rerank");
    }
}
