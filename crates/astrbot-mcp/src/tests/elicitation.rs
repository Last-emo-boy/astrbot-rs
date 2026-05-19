use std::collections::BTreeMap;

use crate::{
    McpElicitationAction, McpElicitationCoordinator, McpElicitationField, McpElicitationRequest,
    McpElicitationResult, McpElicitationSchema, McpElicitationValue, parse_form_reply,
    parse_llm_fallback_json, parse_url_reply,
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

#[test]
fn elicitation_parses_form_url_keywords_and_llm_fallback_json() {
    let schema = McpElicitationSchema::new()
        .with_field(
            "mode",
            McpElicitationField::string()
                .with_enum_value("fast")
                .with_enum_value("safe"),
        )
        .with_field("confirmed", McpElicitationField::boolean())
        .require("mode");

    let parsed =
        parse_form_reply(&schema, "mode: safe\nconfirmed: yes").expect("form reply should parse");
    assert_eq!(parsed.action, McpElicitationAction::Accept);
    assert_eq!(
        parsed.content.get("confirmed"),
        Some(&McpElicitationValue::Boolean(true))
    );

    let declined = parse_form_reply(&schema, "decline").expect("decline should parse");
    assert_eq!(declined.action, McpElicitationAction::Decline);

    let accepted_url = parse_url_reply("done").expect("url action should parse");
    assert_eq!(accepted_url.action, McpElicitationAction::Accept);

    let fallback = parse_llm_fallback_json(
        &schema,
        "```json\n{\"mode\":\"fast\",\"confirmed\":false}\n```",
    )
    .expect("fallback should produce a parse result")
    .expect("fallback JSON should parse");
    assert_eq!(
        fallback.content.get("mode"),
        Some(&McpElicitationValue::String("fast".to_string()))
    );
}

#[test]
fn elicitation_coordinator_enforces_per_umo_lock_and_finishes_sessions() {
    let schema = McpElicitationSchema::new()
        .with_field("mode", McpElicitationField::string())
        .require("mode");
    let mut coordinator = McpElicitationCoordinator::new("docs");

    let session = coordinator
        .begin(
            "umo-1",
            "sender-1",
            McpElicitationRequest::form("choose", schema),
            30,
        )
        .expect("first session should start");
    assert_eq!(session.unified_msg_origin, "umo-1");
    assert!(session.prompt.contains("MCP server `docs`"));

    let blocked = coordinator.begin(
        "umo-1",
        "sender-1",
        McpElicitationRequest::url("open", "https://example.test"),
        30,
    );
    assert!(
        blocked
            .expect_err("same UMO should be locked")
            .to_string()
            .contains("already active")
    );

    let result = coordinator
        .handle_reply("umo-1", "sender-1", "mode: safe", None)
        .expect("reply should finish session");
    assert_eq!(result.action, McpElicitationAction::Accept);
    assert!(!coordinator.has_active_session("umo-1"));
}

#[test]
fn elicitation_coordinator_allows_parallel_sessions_for_different_umos_and_cancel() {
    let mut coordinator = McpElicitationCoordinator::new("docs");
    coordinator
        .begin(
            "umo-1",
            "sender-1",
            McpElicitationRequest::url("open", "https://example.test/one"),
            30,
        )
        .expect("first UMO should start");
    coordinator
        .begin(
            "umo-2",
            "sender-2",
            McpElicitationRequest::url("open", "https://example.test/two"),
            30,
        )
        .expect("second UMO should start");

    let result = coordinator
        .handle_reply("umo-1", "sender-1", "cancel", None)
        .expect("cancel should finish first session");
    assert_eq!(result.action, McpElicitationAction::Cancel);
    assert!(!coordinator.has_active_session("umo-1"));
    assert!(coordinator.has_active_session("umo-2"));

    let result = coordinator
        .handle_reply("umo-2", "sender-2", "done", None)
        .expect("done should finish second session");
    assert_eq!(result.action, McpElicitationAction::Accept);
}

#[test]
fn elicitation_coordinator_cancels_timed_out_sessions() {
    let mut coordinator = McpElicitationCoordinator::new("docs");
    coordinator
        .begin(
            "umo-1",
            "sender-1",
            McpElicitationRequest::url("open", "https://example.test"),
            1,
        )
        .expect("session should start");

    std::thread::sleep(std::time::Duration::from_millis(1100));
    let result = coordinator
        .cancel_expired("umo-1")
        .expect("expired session should cancel");
    assert_eq!(result.action, McpElicitationAction::Cancel);
    assert!(!coordinator.has_active_session("umo-1"));
}
