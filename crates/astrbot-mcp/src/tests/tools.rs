use crate::{McpJsonValue, McpToolCallRequest};

#[test]
fn tool_arguments_are_typed_json_objects_not_value_maps() {
    let request = McpToolCallRequest::new("weather")
        .with_argument("city", "Shanghai")
        .with_argument("days", 3_i32)
        .with_argument("details", true);

    assert_eq!(
        request.arguments.get("city").and_then(McpJsonValue::as_str),
        Some("Shanghai")
    );

    let json = serde_json::to_value(&request).expect("tool call should serialize");
    assert_eq!(json["name"], "weather");
    assert_eq!(json["arguments"]["days"], 3);

    let decoded: McpToolCallRequest =
        serde_json::from_value(json).expect("tool call should deserialize");
    assert_eq!(
        decoded.arguments.get("details"),
        Some(&McpJsonValue::Bool(true))
    );
}
