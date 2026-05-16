#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentFallbackPolicy {
    pub enabled: bool,
    pub require_wake: bool,
    pub error_message: Option<String>,
}

impl Default for AgentFallbackPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            require_wake: false,
            error_message: Some("LLM 请求失败，请稍后再试。".to_string()),
        }
    }
}

impl AgentFallbackPolicy {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    pub fn require_wake(mut self, require_wake: bool) -> Self {
        self.require_wake = require_wake;
        self
    }

    pub fn with_error_message(mut self, error_message: impl Into<String>) -> Self {
        self.error_message = non_empty_option(error_message);
        self
    }

    pub fn without_error_message(mut self) -> Self {
        self.error_message = None;
        self
    }
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then_some(value)
}
