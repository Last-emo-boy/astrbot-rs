use std::collections::BTreeMap;

use crate::{
    McpElicitationField, McpElicitationRequest, McpElicitationResult, McpElicitationSchema,
    McpElicitationValue,
};

#[test]
fn elicitation_schema_and_result_are_typed() {
    let schema = McpElicitationSchema::new()
        .with_field(
            "mode",
            McpElicitationField::string()
                .with_description("Execution mode")
                .with_enum_value("fast")
                .with_enum_value("safe"),
        )
        .with_field("confirmed", McpElicitationField::boolean())
        .require("mode");
    let request = McpElicitationRequest::form("choose", schema.clone());

    let json = serde_json::to_value(&request).expect("elicitation should serialize");
    assert_eq!(json["kind"], "form");
    assert_eq!(json["requested_schema"]["required"][0], "mode");

    let mut content = BTreeMap::new();
    content.insert(
        "mode".to_string(),
        McpElicitationValue::String("safe".to_string()),
    );
    content.insert("confirmed".to_string(), McpElicitationValue::Boolean(true));

    let accepted = McpElicitationResult::accept(content);
    let json = serde_json::to_value(accepted).expect("result should serialize");
    assert_eq!(json["action"], "accept");
    assert_eq!(json["content"]["confirmed"], true);

    let schema_json = schema.into_json_schema();
    assert_eq!(
        schema_json.as_json()["properties"]["mode"]["type"],
        "string"
    );
}
