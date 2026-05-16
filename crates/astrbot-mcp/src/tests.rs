use std::collections::BTreeMap;

use super::*;

#[test]
fn server_config_normalizes_http_transport_and_capabilities() {
    let config = McpServerConfig::default()
        .with_client_capabilities(McpClientCapabilities {
            elicitation: McpElicitationCapabilityConfig {
                enabled: true,
                timeout_seconds: 0,
            },
            sampling: McpSamplingCapabilityConfig { enabled: true },
            roots: McpRootsCapabilityConfig {
                enabled: true,
                paths: vec!["data".to_string(), "".to_string(), "temp".to_string()],
            },
        })
        .with_arg("--serve");

    let normalized = McpServerConfig {
        url: Some(" https://example.invalid/mcp ".to_string()),
        ..config
    }
    .normalize();

    assert_eq!(normalized.transport, McpTransport::Sse);
    assert_eq!(
        normalized.client_capabilities.elicitation.timeout_seconds,
        config::DEFAULT_MCP_ELICITATION_TIMEOUT_SECONDS
    );
    assert_eq!(
        normalized.client_capabilities.roots.paths,
        vec!["data".to_string(), "temp".to_string()]
    );
    assert!(
        normalized
            .client_capabilities
            .supports_interactive_requests()
    );
}

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

#[test]
fn bridge_tool_names_match_sanitized_server_scope() {
    assert_eq!(
        build_mcp_resource_tool_names("My MCP.Server", true),
        vec![
            "mcp_my_mcp_server_list_resources",
            "mcp_my_mcp_server_read_resource",
            "mcp_my_mcp_server_list_resource_templates"
        ]
    );
    assert_eq!(
        build_mcp_prompt_tool_names("My MCP.Server"),
        vec![
            "mcp_my_mcp_server_list_prompts",
            "mcp_my_mcp_server_get_prompt"
        ]
    );
}

#[test]
fn resources_shape_read_result_as_tool_text() {
    let uri = McpUri::new("file:///tmp/readme.txt").expect("uri");
    let result = McpReadResourceResult::text(uri.clone(), "hello");

    let shaped = result.into_tool_result("docs", &uri);

    assert_eq!(shaped.status, McpToolResultStatus::Completed);
    assert!(!shaped.is_error);
    assert_eq!(
        shaped.content,
        vec![McpContentBlock::Text {
            text: "MCP text resource from server 'docs':\nURI: file:///tmp/readme.txt\n\nhello"
                .to_string()
        }]
    );
}

#[test]
fn prompts_and_sampling_use_mcp_json_field_names() {
    let request = McpSamplingRequest {
        messages: vec![McpSamplingMessage::user_text("summarize")],
        system_prompt: Some("be concise".to_string()),
        max_tokens: Some(128),
        temperature: Some(0.2),
        stop_sequences: vec!["END".to_string()],
        include_context: Some(sampling::McpIncludeContext::None),
        metadata: McpJsonObject::new().with("trace", "abc"),
        model_preferences: vec![McpModelHint::new("gpt")],
    };

    let json = serde_json::to_value(request).expect("sampling should serialize");

    assert_eq!(json["systemPrompt"], "be concise");
    assert_eq!(json["maxTokens"], 128);
    assert_eq!(json["stopSequences"][0], "END");
    assert_eq!(json["messages"][0]["content"]["type"], "text");
}

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
fn roots_keep_aliases_and_uri_typed() {
    let defaults = McpRootsCapabilityConfig::enabled_for_default_paths();
    assert_eq!(defaults.paths, vec!["data".to_string(), "temp".to_string()]);
    assert!(
        McpRootAlias::all()
            .iter()
            .any(|alias| alias.as_str() == "knowledge_base")
    );

    let root = McpRoot::new(McpUri::new("file:///tmp").expect("uri")).named("temp");
    let json = serde_json::to_value(root).expect("root should serialize");

    assert_eq!(json["uri"], "file:///tmp");
    assert_eq!(json["name"], "temp");
}
