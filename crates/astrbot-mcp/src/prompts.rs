use serde::{Deserialize, Serialize};

use crate::resources::sanitize_tool_name_fragment;
use crate::tools::{McpContentBlock, McpToolCallResult};
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

pub fn shape_get_prompt_result(
    server_name: &str,
    prompt_name: &str,
    response: &McpGetPromptResult,
) -> McpToolCallResult {
    let mut lines = vec![
        format!("MCP prompt from server '{server_name}':"),
        format!("Prompt: {prompt_name}"),
    ];
    if let Some(description) = &response.description {
        lines.push(format!("Description: {description}"));
    }
    if response.messages.is_empty() {
        lines.push("No prompt messages returned.".to_string());
        return McpToolCallResult::text(lines.join("\n"));
    }

    lines.push("Messages:".to_string());
    for (idx, message) in response.messages.iter().enumerate() {
        lines.push(format!("{}. {:?}", idx + 1, message.role).to_lowercase());
        lines.extend(format_prompt_message_content(&message.content));
    }
    McpToolCallResult::text(lines.join("\n"))
}

fn format_prompt_message_content(content: &McpContentBlock) -> Vec<String> {
    match content {
        McpContentBlock::Text { text } => {
            let lines = text.lines().map(str::to_string).collect::<Vec<_>>();
            if lines.is_empty() {
                vec![text.clone()]
            } else {
                lines
            }
        }
        McpContentBlock::Image { data, mime_type } => vec![
            "Image block returned.".to_string(),
            format!("MIME type: {}", mime_type.as_str()),
            format!("Base64 length: {}", data.len()),
        ],
        McpContentBlock::Audio { data, mime_type } => vec![
            "Audio block returned.".to_string(),
            format!("MIME type: {}", mime_type.as_str()),
            format!("Base64 length: {}", data.len()),
        ],
        McpContentBlock::Resource { resource } => match &resource.resource {
            crate::resources::McpResourceContent::Text {
                uri,
                text,
                mime_type,
            } => {
                let mut lines = vec![
                    "Embedded text resource returned.".to_string(),
                    format!("URI: {}", uri.as_str()),
                ];
                if let Some(mime_type) = mime_type {
                    lines.push(format!("MIME type: {}", mime_type.as_str()));
                }
                lines.extend(text.lines().map(str::to_string));
                lines
            }
            crate::resources::McpResourceContent::Blob {
                uri,
                blob,
                mime_type,
            } => {
                let mut lines = vec![
                    "Embedded binary resource returned.".to_string(),
                    format!("URI: {}", uri.as_str()),
                ];
                if let Some(mime_type) = mime_type {
                    lines.push(format!("MIME type: {}", mime_type.as_str()));
                }
                lines.push(format!("Base64 length: {}", blob.len()));
                lines
            }
        },
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPromptListRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<McpCursor>,
}
