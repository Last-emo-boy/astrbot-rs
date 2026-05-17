use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderToolCall {
    pub id: String,
    pub name: String,
    pub arguments: ProviderToolCallArguments,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_content: Option<Value>,
}

impl ProviderToolCall {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: ProviderToolCallArguments,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
            extra_content: None,
        }
    }

    pub fn with_extra_content(mut self, extra_content: Value) -> Self {
        self.extra_content = Some(extra_content);
        self
    }

    pub fn arguments_json(&self) -> String {
        self.arguments.as_json_string()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ProviderToolCallArguments {
    Json(Value),
    PartialJson(String),
    Empty,
}

impl ProviderToolCallArguments {
    pub fn from_raw(raw: &str) -> Self {
        let raw = raw.trim();
        if raw.is_empty() {
            return Self::Empty;
        }

        serde_json::from_str(raw)
            .map(Self::Json)
            .unwrap_or_else(|_| Self::PartialJson(raw.to_string()))
    }

    pub fn as_json_string(&self) -> String {
        match self {
            Self::Json(value) => value.to_string(),
            Self::PartialJson(value) => value.clone(),
            Self::Empty => "{}".to_string(),
        }
    }
}
