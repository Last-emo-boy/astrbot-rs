use serde_json::{Value, json};

use crate::ToolDescriptor;

use super::ToolSchemaSerializer;

#[derive(Clone, Debug, Default)]
pub struct AnthropicToolSchemaSerializer;

impl ToolSchemaSerializer for AnthropicToolSchemaSerializer {
    fn serialize_tools(&self, tools: &[ToolDescriptor]) -> Value {
        Value::Array(
            tools
                .iter()
                .map(|tool| {
                    let mut schema = json!({
                        "name": tool.name,
                        "input_schema": {
                            "type": "object",
                            "properties": tool.parameters.get("properties").cloned().unwrap_or_else(|| json!({})),
                            "required": tool.parameters.get("required").cloned().unwrap_or_else(|| json!([]))
                        }
                    });
                    if let Some(description) = &tool.description {
                        schema["description"] = json!(description);
                    }
                    schema
                })
                .collect(),
        )
    }
}
