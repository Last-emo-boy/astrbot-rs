use serde::{Deserialize, Serialize};

use crate::tools::McpContentBlock;
use crate::types::{McpJsonObject, McpJsonValue};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpSamplingRole {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSamplingMessage {
    pub role: McpSamplingRole,
    pub content: McpContentBlock,
}

impl McpSamplingMessage {
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: McpSamplingRole::User,
            content: McpContentBlock::Text { text: text.into() },
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSamplingRequest {
    pub messages: Vec<McpSamplingMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop_sequences: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_context: Option<McpIncludeContext>,
    #[serde(default, skip_serializing_if = "McpJsonObject::is_empty")]
    pub metadata: McpJsonObject,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_preferences: Vec<McpModelHint>,
}

impl McpSamplingRequest {
    pub fn new(messages: Vec<McpSamplingMessage>) -> Self {
        Self {
            messages,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpIncludeContext {
    None,
    ThisServer,
    AllServers,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpModelHint {
    pub name: String,
}

impl McpModelHint {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSamplingResult {
    pub role: McpSamplingRole,
    pub content: McpContentBlock,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(default, skip_serializing_if = "McpJsonObject::is_empty")]
    pub metadata: McpJsonObject,
}

impl McpSamplingResult {
    pub fn assistant_text(text: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            role: McpSamplingRole::Assistant,
            content: McpContentBlock::Text { text: text.into() },
            model: model.into(),
            stop_reason: None,
            metadata: McpJsonObject::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<McpJsonValue>) -> Self {
        self.metadata = self.metadata.with(key, value);
        self
    }
}
