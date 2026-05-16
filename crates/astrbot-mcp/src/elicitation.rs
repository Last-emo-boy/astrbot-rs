use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::types::{McpJsonSchema, McpJsonValue};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpElicitationRequest {
    Form {
        message: String,
        requested_schema: McpElicitationSchema,
    },
    Url {
        message: String,
        url: String,
    },
}

impl McpElicitationRequest {
    pub fn form(message: impl Into<String>, requested_schema: McpElicitationSchema) -> Self {
        Self::Form {
            message: message.into(),
            requested_schema,
        }
    }

    pub fn url(message: impl Into<String>, url: impl Into<String>) -> Self {
        Self::Url {
            message: message.into(),
            url: url.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpElicitationSchema {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, McpElicitationField>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
}

impl McpElicitationSchema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_field(mut self, name: impl Into<String>, field: McpElicitationField) -> Self {
        let name = name.into().trim().to_string();
        if !name.is_empty() {
            self.properties.insert(name, field);
        }
        self
    }

    pub fn require(mut self, name: impl Into<String>) -> Self {
        let name = name.into().trim().to_string();
        if !name.is_empty() && !self.required.contains(&name) {
            self.required.push(name);
        }
        self
    }

    pub fn into_json_schema(self) -> McpJsonSchema {
        let value = serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}));
        McpJsonSchema::from_json(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpElicitationField {
    #[serde(rename = "type")]
    pub field_type: McpElicitationFieldType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<McpElicitationValue>,
}

impl McpElicitationField {
    pub fn string() -> Self {
        Self {
            field_type: McpElicitationFieldType::String,
            description: None,
            enum_values: Vec::new(),
        }
    }

    pub fn integer() -> Self {
        Self {
            field_type: McpElicitationFieldType::Integer,
            description: None,
            enum_values: Vec::new(),
        }
    }

    pub fn boolean() -> Self {
        Self {
            field_type: McpElicitationFieldType::Boolean,
            description: None,
            enum_values: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.trim().is_empty()).then_some(description);
        self
    }

    pub fn with_enum_value(mut self, value: impl Into<McpElicitationValue>) -> Self {
        self.enum_values.push(value.into());
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpElicitationFieldType {
    String,
    Integer,
    Number,
    Boolean,
    Array,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpElicitationValue {
    String(String),
    Integer(i64),
    Number(f64),
    Boolean(bool),
    StringArray(Vec<String>),
    Null,
}

impl From<&str> for McpElicitationValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for McpElicitationValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<i64> for McpElicitationValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<bool> for McpElicitationValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<McpElicitationValue> for McpJsonValue {
    fn from(value: McpElicitationValue) -> Self {
        match value {
            McpElicitationValue::String(value) => Self::String(value),
            McpElicitationValue::Integer(value) => Self::Integer(value),
            McpElicitationValue::Number(value) => Self::Number(value),
            McpElicitationValue::Boolean(value) => Self::Bool(value),
            McpElicitationValue::StringArray(value) => {
                Self::Array(value.into_iter().map(McpJsonValue::String).collect())
            }
            McpElicitationValue::Null => Self::Null,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpElicitationResult {
    pub action: McpElicitationAction,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub content: BTreeMap<String, McpElicitationValue>,
}

impl McpElicitationResult {
    pub fn accept(content: BTreeMap<String, McpElicitationValue>) -> Self {
        Self {
            action: McpElicitationAction::Accept,
            content,
        }
    }

    pub fn decline() -> Self {
        Self {
            action: McpElicitationAction::Decline,
            content: BTreeMap::new(),
        }
    }

    pub fn cancel() -> Self {
        Self {
            action: McpElicitationAction::Cancel,
            content: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpElicitationAction {
    Accept,
    Decline,
    Cancel,
}
