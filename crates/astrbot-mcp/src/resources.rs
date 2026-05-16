use serde::{Deserialize, Serialize};

use crate::tools::McpToolCallResult;
use crate::types::{McpCursor, McpMimeType, McpUri};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResource {
    pub uri: McpUri,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<McpMimeType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

impl McpResource {
    pub fn new(uri: McpUri, name: impl Into<String>) -> Self {
        Self {
            uri,
            name: name.into(),
            title: None,
            description: None,
            mime_type: None,
            size: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceTemplate {
    #[serde(rename = "uriTemplate")]
    pub uri_template: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<McpMimeType>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpReadResourceRequest {
    pub uri: McpUri,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

impl McpReadResourceRequest {
    pub fn new(uri: McpUri) -> Self {
        Self {
            uri,
            timeout_seconds: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpReadResourceResult {
    #[serde(default)]
    pub contents: Vec<McpResourceContent>,
}

impl McpReadResourceResult {
    pub fn text(uri: McpUri, text: impl Into<String>) -> Self {
        Self {
            contents: vec![McpResourceContent::Text {
                uri,
                text: text.into(),
                mime_type: None,
            }],
        }
    }

    pub fn into_tool_result(self, server_name: &str, requested_uri: &McpUri) -> McpToolCallResult {
        if self.contents.is_empty() {
            return McpToolCallResult::text(format!(
                "MCP server '{server_name}' returned no contents for resource '{}'.",
                requested_uri.as_str()
            ));
        }

        if self.contents.len() == 1
            && let McpResourceContent::Text {
                uri,
                text,
                mime_type,
            } = &self.contents[0]
        {
            let mut lines = vec![
                format!("MCP text resource from server '{server_name}':"),
                format!("URI: {}", uri.as_str()),
            ];
            if let Some(mime_type) = mime_type {
                lines.push(format!("MIME type: {}", mime_type.as_str()));
            }
            lines.push(String::new());
            lines.push(text.clone());
            return McpToolCallResult::text(lines.join("\n"));
        }

        McpToolCallResult::text(format!(
            "MCP resource read result from server '{server_name}':\nRequested URI: {}\nReturned parts: {}",
            requested_uri.as_str(),
            self.contents.len()
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum McpResourceContent {
    Text {
        uri: McpUri,
        text: String,
        #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
        mime_type: Option<McpMimeType>,
    },
    Blob {
        uri: McpUri,
        blob: String,
        #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
        mime_type: Option<McpMimeType>,
    },
}

pub fn build_mcp_resource_tool_names(server_name: &str, include_templates: bool) -> Vec<String> {
    let safe_server_name = sanitize_tool_name_fragment(server_name);
    let mut names = vec![
        format!("mcp_{safe_server_name}_list_resources"),
        format!("mcp_{safe_server_name}_read_resource"),
    ];
    if include_templates {
        names.push(format!("mcp_{safe_server_name}_list_resource_templates"));
    }
    names
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceListRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<McpCursor>,
}

pub(crate) fn sanitize_tool_name_fragment(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len());
    let mut previous_underscore = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
            previous_underscore = false;
        } else if !previous_underscore {
            sanitized.push('_');
            previous_underscore = true;
        }
    }
    let sanitized = sanitized.trim_matches('_').to_string();
    if sanitized.is_empty() {
        "server".to_string()
    } else {
        sanitized
    }
}
