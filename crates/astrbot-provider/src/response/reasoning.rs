use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderReasoningMetadata {
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl ProviderReasoningMetadata {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            signature: None,
        }
    }

    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        let signature = signature.into();
        self.signature = (!signature.trim().is_empty()).then_some(signature);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty() && self.signature.is_none()
    }
}
