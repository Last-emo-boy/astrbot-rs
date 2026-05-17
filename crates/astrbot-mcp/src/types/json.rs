use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{McpError, McpResult};

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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{McpJsonObject, McpJsonValue};

    #[test]
    fn json_value_round_trips_nested_objects_without_client_lifecycle() {
        let value = McpJsonValue::try_from(json!({
            "city": "Shanghai",
            "days": 3,
            "details": true,
            "tags": ["weather"]
        }))
        .expect("typed json should decode");

        let object = value.as_object().expect("json should be object");
        assert_eq!(
            object.get("city").and_then(McpJsonValue::as_str),
            Some("Shanghai")
        );

        let encoded = serde_json::Value::from(value);
        assert_eq!(encoded["days"], 3);
        assert_eq!(encoded["tags"][0], "weather");
    }

    #[test]
    fn json_object_ignores_blank_keys() {
        let object = McpJsonObject::new().with(" ", true).with("ok", false);

        assert_eq!(object.0.len(), 1);
        assert_eq!(object.get("ok"), Some(&McpJsonValue::Bool(false)));
    }
}
