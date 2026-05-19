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

    if let Some(any_of) = input.get("anyOf").and_then(Value::as_array) {
        return json!({
            "anyOf": any_of.iter().map(convert_schema).collect::<Vec<_>>()
        });
    }

    let mut output = Map::new();
    let schema_type = normalized_type(schema);
    output.insert("type".to_string(), schema_type.clone());
    if let (Some(kind), Some(format)) = (
        schema_type.as_str(),
        input.get("format").and_then(Value::as_str),
    ) {
        if supports_format(kind, format) {
            output.insert("format".to_string(), json!(format));
        }
    }

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

fn supports_format(kind: &str, format: &str) -> bool {
    matches!(
        (kind, format),
        ("string", "enum")
            | ("string", "date-time")
            | ("integer", "int32")
            | ("integer", "int64")
            | ("number", "float")
            | ("number", "double")
    )
}
