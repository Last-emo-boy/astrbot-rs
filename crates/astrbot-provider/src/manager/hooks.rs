use crate::{ProviderSelection, ProviderSelectionScope};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderSelectionChanged {
    pub selection: ProviderSelection,
    pub previous_provider_id: Option<String>,
}

impl ProviderSelectionChanged {
    pub fn new(selection: ProviderSelection, previous_provider_id: Option<String>) -> Self {
        Self {
            selection,
            previous_provider_id,
        }
    }

    pub fn is_session_override(&self) -> bool {
        matches!(self.selection.scope, ProviderSelectionScope::Session { .. })
    }
}

pub trait ProviderSelectionHook: Send + Sync {
    fn provider_selection_changed(&self, event: &ProviderSelectionChanged);
}

pub struct NoopProviderSelectionHook;

impl ProviderSelectionHook for NoopProviderSelectionHook {
    fn provider_selection_changed(&self, _event: &ProviderSelectionChanged) {}
}

#[cfg(test)]
mod tests {
    use crate::{ProviderCapability, ProviderSelection, ProviderSelectionChanged};

    #[test]
    fn selection_change_marks_session_overrides() {
        let default_change = ProviderSelectionChanged::new(
            ProviderSelection::default_provider(ProviderCapability::ChatCompletion, "chat"),
            None,
        );
        let session_change = ProviderSelectionChanged::new(
            ProviderSelection::session_provider(
                ProviderCapability::ChatCompletion,
                "session-1",
                "chat-session",
            ),
            Some("chat".to_string()),
        );

        assert!(!default_change.is_session_override());
        assert!(session_change.is_session_override());
    }
}
