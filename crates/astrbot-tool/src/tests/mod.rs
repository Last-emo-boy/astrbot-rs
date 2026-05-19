mod catalog;
mod commands;
mod conflicts;
mod internal;
mod parity;
mod reference;
mod schema;
mod source;
mod web_search;

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
