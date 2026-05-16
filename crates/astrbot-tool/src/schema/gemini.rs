use serde_json::{Map, Value, json};

use crate::ToolDescriptor;

use super::ToolSchemaSerializer;

#[derive(Clone, Debug, Default)]
pub struct GeminiToolSchemaSerializer;

impl ToolSchemaSerializer for GeminiToolSchemaSerializer {
    fn serialize_tools(&self, tools: &[ToolDescriptor]) -> Value {
        let declarations = tools
            .iter()
            .map(|tool| {
                let mut declaration = json!({ "name": tool.name });
                if let Some(description) = &tool.description {
                    declaration["description"] = json!(description);
                }
                declaration["parameters"] = convert_schema(&tool.parameters);
                declaration
            })
            .collect::<Vec<_>>();

        if declarations.is_empty() {
            json!({})
        } else {
            json!({ "function_declarations": declarations })
        }
    }
}

fn convert_schema(schema: &Value) -> Value {
    let Some(input) = schema.as_object() else {
        return json!({ "type": "null" });
    };

    let mut output = Map::new();
    output.insert("type".to_string(), normalized_type(schema));

    for key in [
        "title",
        "description",
        "enum",
        "minimum",
        "maximum",
        "maxItems",
        "minItems",
        "nullable",
        "required",
    ] {
        if let Some(value) = input.get(key) {
            output.insert(key.to_string(), value.clone());
        }
    }

    if let Some(properties) = input.get("properties").and_then(Value::as_object) {
        let converted = properties
            .iter()
            .map(|(name, value)| {
                let mut property = convert_schema(value);
                if let Some(property_object) = property.as_object_mut() {
                    property_object.remove("default");
                    property_object.remove("additionalProperties");
                }
                (name.clone(), property)
            })
            .collect::<Map<_, _>>();
        if !converted.is_empty() {
            output.insert("properties".to_string(), Value::Object(converted));
        }
    }

    if let Some(items) = input.get("items") {
        output.insert("items".to_string(), convert_schema(items));
    }

    Value::Object(output)
}

fn normalized_type(schema: &Value) -> Value {
    let Some(kind) = schema.get("type") else {
        return json!("object");
    };

    if let Some(kind) = kind.as_str() {
        return supported_type(kind);
    }

    if let Some(kinds) = kind.as_array() {
        let selected = kinds
            .iter()
            .filter_map(Value::as_str)
            .find(|kind| *kind != "null")
            .unwrap_or("string");
        return supported_type(selected);
    }

    json!("null")
}

fn supported_type(kind: &str) -> Value {
    match kind {
        "string" | "number" | "integer" | "boolean" | "array" | "object" | "null" => {
            json!(kind)
        }
        _ => json!("null"),
    }
}
