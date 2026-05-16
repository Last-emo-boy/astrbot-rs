use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type McpResult<T> = std::result::Result<T, McpError>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum McpError {
    #[error("invalid MCP config: {0}")]
    InvalidConfig(String),

    #[error("MCP client is not connected: {0}")]
    NotConnected(String),

    #[error("unsupported MCP request: {0}")]
    Unsupported(String),

    #[error("MCP transport error: {0}")]
    Transport(String),

    #[error("MCP protocol error: {0}")]
    Protocol(String),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpServerName(String);

impl McpServerName {
    pub fn new(value: impl Into<String>) -> McpResult<Self> {
        let value = value.into().trim().to_string();
        if value.is_empty() {
            return Err(McpError::InvalidConfig(
                "server name cannot be empty".to_string(),
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for McpServerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for McpServerName {
    type Error = McpError;

    fn try_from(value: String) -> McpResult<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for McpServerName {
    type Error = McpError;

    fn try_from(value: &str) -> McpResult<Self> {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpUri(String);

impl McpUri {
    pub fn new(value: impl Into<String>) -> McpResult<Self> {
        let value = value.into().trim().to_string();
        if value.is_empty() {
            return Err(McpError::InvalidConfig("uri cannot be empty".to_string()));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for McpUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for McpUri {
    type Error = McpError;

    fn try_from(value: String) -> McpResult<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for McpUri {
    type Error = McpError;

    fn try_from(value: &str) -> McpResult<Self> {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpCursor(String);

impl McpCursor {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into().trim().to_string();
        (!value.is_empty()).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpMimeType(String);

impl McpMimeType {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into().trim().to_string();
        (!value.is_empty()).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpJsonValue {
    Null,
    Bool(bool),
    Integer(i64),
    Number(f64),
    String(String),
    Array(Vec<McpJsonValue>),
    Object(McpJsonObject),
}

impl McpJsonValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&McpJsonObject> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }
}

impl From<&str> for McpJsonValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for McpJsonValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<bool> for McpJsonValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for McpJsonValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<i32> for McpJsonValue {
    fn from(value: i32) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<f64> for McpJsonValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<McpJsonObject> for McpJsonValue {
    fn from(value: McpJsonObject) -> Self {
        Self::Object(value)
    }
}

impl TryFrom<serde_json::Value> for McpJsonValue {
    type Error = McpError;

    fn try_from(value: serde_json::Value) -> McpResult<Self> {
        match value {
            serde_json::Value::Null => Ok(Self::Null),
            serde_json::Value::Bool(value) => Ok(Self::Bool(value)),
            serde_json::Value::Number(value) => {
                if let Some(integer) = value.as_i64() {
                    Ok(Self::Integer(integer))
                } else if let Some(number) = value.as_f64() {
                    Ok(Self::Number(number))
                } else {
                    Err(McpError::Protocol("unsupported JSON number".to_string()))
                }
            }
            serde_json::Value::String(value) => Ok(Self::String(value)),
            serde_json::Value::Array(values) => values
                .into_iter()
                .map(Self::try_from)
                .collect::<McpResult<Vec<_>>>()
                .map(Self::Array),
            serde_json::Value::Object(values) => values
                .into_iter()
                .map(|(key, value)| Self::try_from(value).map(|value| (key, value)))
                .collect::<McpResult<BTreeMap<_, _>>>()
                .map(McpJsonObject)
                .map(Self::Object),
        }
    }
}

impl From<McpJsonValue> for serde_json::Value {
    fn from(value: McpJsonValue) -> Self {
        match value {
            McpJsonValue::Null => Self::Null,
            McpJsonValue::Bool(value) => Self::Bool(value),
            McpJsonValue::Integer(value) => Self::Number(value.into()),
            McpJsonValue::Number(value) => serde_json::Number::from_f64(value)
                .map(Self::Number)
                .unwrap_or(Self::Null),
            McpJsonValue::String(value) => Self::String(value),
            McpJsonValue::Array(values) => {
                Self::Array(values.into_iter().map(serde_json::Value::from).collect())
            }
            McpJsonValue::Object(values) => values.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpJsonObject(pub BTreeMap<String, McpJsonValue>);

impl McpJsonObject {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, key: impl Into<String>, value: impl Into<McpJsonValue>) -> Self {
        let key = key.into().trim().to_string();
        if !key.is_empty() {
            self.0.insert(key, value.into());
        }
        self
    }

    pub fn get(&self, key: &str) -> Option<&McpJsonValue> {
        self.0.get(key)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<McpJsonObject> for serde_json::Value {
    fn from(value: McpJsonObject) -> Self {
        let object = value
            .0
            .into_iter()
            .map(|(key, value)| (key, serde_json::Value::from(value)))
            .collect();
        Self::Object(object)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpJsonSchema(pub serde_json::Value);

impl Default for McpJsonSchema {
    fn default() -> Self {
        Self::object()
    }
}

impl McpJsonSchema {
    pub fn object() -> Self {
        Self(serde_json::json!({
            "type": "object",
            "properties": {}
        }))
    }

    pub fn from_json(value: serde_json::Value) -> Self {
        Self(value)
    }

    pub fn as_json(&self) -> &serde_json::Value {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpListPage<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<McpCursor>,
}

impl<T> McpListPage<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            next_cursor: None,
        }
    }

    pub fn with_next_cursor(mut self, next_cursor: impl Into<String>) -> Self {
        self.next_cursor = McpCursor::new(next_cursor);
        self
    }
}
