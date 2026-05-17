use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::McpJsonSchema;

    #[test]
    fn default_schema_is_empty_object_schema() {
        let schema = McpJsonSchema::default();

        assert_eq!(schema.as_json()["type"], "object");
        assert!(schema.as_json()["properties"].is_object());
    }
}
