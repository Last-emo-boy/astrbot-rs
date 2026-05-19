use serde::{Deserialize, Serialize};

use crate::tools::McpContentBlock;
use crate::types::{McpError, McpJsonObject, McpJsonValue, McpResult};

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<McpJsonObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<McpJsonValue>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpSamplingInteractionState {
    pub sampling_enabled: bool,
    pub active_interaction: bool,
    pub provider_available: bool,
    pub unified_msg_origin: Option<String>,
}

impl McpSamplingInteractionState {
    pub fn active(umo: impl Into<String>) -> Self {
        Self {
            sampling_enabled: true,
            active_interaction: true,
            provider_available: true,
            unified_msg_origin: Some(umo.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpProviderSamplingRequest {
    pub contexts: Vec<McpProviderSamplingContext>,
    pub system_prompt: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stop_sequences: Vec<String>,
    pub metadata: McpJsonObject,
    pub unified_msg_origin: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpProviderSamplingContext {
    pub role: McpSamplingRole,
    pub content: String,
}

pub struct McpSamplingPolicy;

impl McpSamplingPolicy {
    pub fn prepare_provider_request(
        state: &McpSamplingInteractionState,
        request: &McpSamplingRequest,
    ) -> McpResult<McpProviderSamplingRequest> {
        if !state.sampling_enabled {
            return Err(McpError::Unsupported(
                "Sampling is not enabled for this MCP server.".to_string(),
            ));
        }
        if !state.active_interaction {
            return Err(McpError::Unsupported(
                "Sampling requests are only supported during an active AstrBot MCP interaction."
                    .to_string(),
            ));
        }
        if request
            .include_context
            .is_some_and(|context| context != McpIncludeContext::None)
        {
            return Err(McpError::Unsupported(
                "Sampling includeContext is not supported in the initial AstrBot integration."
                    .to_string(),
            ));
        }
        if !request.tools.is_empty() || request.tool_choice.is_some() {
            return Err(McpError::Unsupported(
                "Tool-assisted sampling is not supported in the initial AstrBot integration."
                    .to_string(),
            ));
        }
        if !state.provider_available {
            return Err(McpError::Unsupported(
                "Sampling requires an active chat provider.".to_string(),
            ));
        }
        let unified_msg_origin = state
            .unified_msg_origin
            .as_deref()
            .map(str::trim)
            .filter(|umo| !umo.is_empty())
            .ok_or_else(|| {
                McpError::Unsupported(
                    "Sampling requires a valid unified message origin.".to_string(),
                )
            })?
            .to_string();
        let contexts = request
            .messages
            .iter()
            .map(Self::translate_message)
            .collect::<McpResult<Vec<_>>>()?;
        Ok(McpProviderSamplingRequest {
            contexts,
            system_prompt: request.system_prompt.clone().unwrap_or_default(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stop_sequences: request.stop_sequences.clone(),
            metadata: request.metadata.clone(),
            unified_msg_origin,
        })
    }

    fn translate_message(message: &McpSamplingMessage) -> McpResult<McpProviderSamplingContext> {
        Ok(McpProviderSamplingContext {
            role: message.role,
            content: sampling_content_to_text(&message.content)?,
        })
    }
}

fn sampling_content_to_text(content: &McpContentBlock) -> McpResult<String> {
    match content {
        McpContentBlock::Text { text } => Ok(text.clone()),
        McpContentBlock::Image { .. } => Err(McpError::Unsupported(
            "Image sampling inputs are not supported in the initial AstrBot integration."
                .to_string(),
        )),
        McpContentBlock::Audio { .. } => Err(McpError::Unsupported(
            "Audio sampling inputs are not supported in the initial AstrBot integration."
                .to_string(),
        )),
        McpContentBlock::Resource { .. } => Err(McpError::Unsupported(
            "Resource sampling inputs are not supported in the initial AstrBot integration."
                .to_string(),
        )),
    }
}
