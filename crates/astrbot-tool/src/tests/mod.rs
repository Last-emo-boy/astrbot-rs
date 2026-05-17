mod catalog;
mod commands;
mod conflicts;
mod internal;
mod reference;
mod schema;
mod source;

use serde_json::json;

use crate::ToolDescriptor;

fn weather_tool() -> ToolDescriptor {
    ToolDescriptor::new("weather")
        .with_description("Get weather")
        .with_parameters(json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "City name"
                }
            },
            "required": ["city"]
        }))
}
