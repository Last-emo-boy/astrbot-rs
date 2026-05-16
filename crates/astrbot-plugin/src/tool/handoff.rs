#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandoffToolTarget {
    pub agent_name: String,
    pub provider_id: Option<String>,
    pub background_allowed: bool,
}

impl HandoffToolTarget {
    pub fn new(agent_name: impl Into<String>) -> Self {
        Self {
            agent_name: agent_name.into(),
            provider_id: None,
            background_allowed: false,
        }
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        let provider_id = provider_id.into();
        self.provider_id = (!provider_id.trim().is_empty()).then_some(provider_id);
        self
    }

    pub fn allow_background(mut self) -> Self {
        self.background_allowed = true;
        self
    }
}
