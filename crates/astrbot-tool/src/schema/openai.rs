use serde_json::{Value, json};

use crate::ToolDescriptor;

use super::ToolSchemaSerializer;

#[derive(Clone, Debug, Default)]
pub struct OpenAiToolSchemaSerializer {
    pub omit_empty_parameters: bool,
}

impl ToolSchemaSerializer for OpenAiToolSchemaSerializer {
    fn serialize_tools(&self, tools: &[ToolDescriptor]) -> Value {
        Value::Array(
            tools
                .iter()
                .map(|tool| {
                    let mut function = json!({ "name": tool.name });
                    if let Some(description) = &tool.description {
                        function["description"] = json!(description);
                    }
                    if !self.omit_empty_parameters || has_properties(&tool.parameters) {
                        function["parameters"] = tool.parameters.clone();
                    }
                    json!({
                        "type": "function",
                        "function": function
                    })
                })
                .collect(),
        )
    }
}

fn has_properties(parameters: &Value) -> bool {
    parameters
        .get("properties")
        .and_then(Value::as_object)
        .is_some_and(|properties| !properties.is_empty())
}
