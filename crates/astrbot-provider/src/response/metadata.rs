use astrbot_core::MessageChain;
use serde::{Deserialize, Serialize};

use super::{ProviderReasoningMetadata, ProviderTokenUsage, ProviderToolCall};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderResponseMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ProviderTokenUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ProviderReasoningMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ProviderToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_response: Option<ProviderRawResponse>,
}

impl ProviderResponseMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_response_id(mut self, response_id: impl Into<String>) -> Self {
        self.response_id = non_empty_option(response_id);
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = non_empty_option(model);
        self
    }

    pub fn with_finish_reason(mut self, finish_reason: impl Into<String>) -> Self {
        self.finish_reason = non_empty_option(finish_reason);
        self
    }

    pub fn with_stop_reason(mut self, stop_reason: impl Into<String>) -> Self {
        self.stop_reason = non_empty_option(stop_reason);
        self
    }

    pub fn with_usage(mut self, usage: ProviderTokenUsage) -> Self {
        self.usage = (!usage.is_empty()).then_some(usage);
        self
    }

    pub fn with_reasoning(mut self, reasoning: ProviderReasoningMetadata) -> Self {
        self.reasoning = (!reasoning.is_empty()).then_some(reasoning);
        self
    }

    pub fn with_tool_call(mut self, tool_call: ProviderToolCall) -> Self {
        self.tool_calls.push(tool_call);
        self
    }

    pub fn with_raw_response(mut self, raw_response: ProviderRawResponse) -> Self {
        self.raw_response = Some(raw_response);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.response_id.is_none()
            && self.model.is_none()
            && self.finish_reason.is_none()
            && self.stop_reason.is_none()
            && self.usage.is_none()
            && self.reasoning.is_none()
            && self.tool_calls.is_empty()
            && self.raw_response.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRawResponse {
    pub provider: String,
    pub payload: serde_json::Value,
}

impl ProviderRawResponse {
    pub fn new(provider: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            provider: provider.into(),
            payload,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderResponse {
    pub chain: MessageChain,
    pub metadata: ProviderResponseMetadata,
}

impl ProviderResponse {
    pub fn new(chain: impl Into<MessageChain>, metadata: ProviderResponseMetadata) -> Self {
        Self {
            chain: chain.into(),
            metadata,
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self {
            chain: MessageChain::plain(text),
            metadata: ProviderResponseMetadata::default(),
        }
    }
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then_some(value)
}
