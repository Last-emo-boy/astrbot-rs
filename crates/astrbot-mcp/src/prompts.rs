use serde::{Deserialize, Serialize};

use crate::resources::sanitize_tool_name_fragment;
use crate::tools::McpContentBlock;
use crate::types::{McpCursor, McpJsonObject};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPrompt {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<McpPromptArgument>,
}

impl McpPrompt {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: None,
            description: None,
            arguments: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPromptArgument {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
}

impl McpPromptArgument {
    pub fn required(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            required: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpGetPromptRequest {
    pub name: String,
    #[serde(default, skip_serializing_if = "McpJsonObject::is_empty")]
    pub arguments: McpJsonObject,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

impl McpGetPromptRequest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            arguments: McpJsonObject::new(),
            timeout_seconds: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpGetPromptResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub messages: Vec<McpPromptMessage>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPromptMessage {
    pub role: crate::sampling::McpSamplingRole,
    pub content: McpContentBlock,
}

pub fn build_mcp_prompt_tool_names(server_name: &str) -> Vec<String> {
    let safe_server_name = sanitize_tool_name_fragment(server_name);
    vec![
        format!("mcp_{safe_server_name}_list_prompts"),
        format!("mcp_{safe_server_name}_get_prompt"),
    ]
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPromptListRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<McpCursor>,
}
