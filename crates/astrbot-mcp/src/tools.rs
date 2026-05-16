use serde::{Deserialize, Serialize};

use crate::resources::McpResourceContent;
use crate::sampling::McpSamplingRole;
use crate::types::{McpJsonObject, McpJsonSchema, McpJsonValue, McpMimeType};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: McpJsonSchema,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<McpAnnotations>,
}

impl McpTool {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: None,
            description: None,
            input_schema: McpJsonSchema::object(),
            annotations: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.trim().is_empty()).then_some(description);
        self
    }

    pub fn with_input_schema(mut self, schema: McpJsonSchema) -> Self {
        self.input_schema = schema;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpAnnotations {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audience: Vec<McpSamplingRole>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
    #[serde(default)]
    pub destructive_hint: bool,
    #[serde(default)]
    pub idempotent_hint: bool,
    #[serde(default)]
    pub open_world_hint: bool,
    #[serde(default)]
    pub read_only_hint: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpToolArguments(pub McpJsonObject);

impl McpToolArguments {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, key: impl Into<String>, value: impl Into<McpJsonValue>) -> Self {
        self.0 = self.0.with(key, value);
        self
    }

    pub fn get(&self, key: &str) -> Option<&McpJsonValue> {
        self.0.get(key)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCallRequest {
    pub name: String,
    #[serde(default, skip_serializing_if = "McpToolArguments::is_empty")]
    pub arguments: McpToolArguments,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

impl McpToolCallRequest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            arguments: McpToolArguments::new(),
            timeout_seconds: None,
        }
    }

    pub fn with_argument(mut self, key: impl Into<String>, value: impl Into<McpJsonValue>) -> Self {
        self.arguments = self.arguments.with(key, value);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCallResult {
    #[serde(default)]
    pub content: Vec<McpContentBlock>,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub status: McpToolResultStatus,
}

impl McpToolCallResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![McpContentBlock::Text { text: text.into() }],
            is_error: false,
            status: McpToolResultStatus::Completed,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![McpContentBlock::Text { text: text.into() }],
            is_error: true,
            status: McpToolResultStatus::Rejected,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpToolResultStatus {
    #[default]
    Completed,
    AcceptedBackground,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum McpContentBlock {
    Text {
        text: String,
    },
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: McpMimeType,
    },
    Audio {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: McpMimeType,
    },
    Resource {
        resource: McpEmbeddedResource,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpEmbeddedResource {
    pub resource: McpResourceContent,
}
