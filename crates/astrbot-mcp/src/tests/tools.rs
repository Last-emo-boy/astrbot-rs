use crate::{
    MCP_JSONRPC_INTERNAL_ERROR, MCP_JSONRPC_INVALID_REQUEST, MCP_JSONRPC_METHOD_NOT_FOUND,
    MCP_JSONRPC_PARSE_ERROR, McpError, McpJsonValue, McpToolCallRequest,
};

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
fn mcp_errors_map_to_json_rpc_error_codes() {
    assert_eq!(
        McpError::InvalidConfig("bad".to_string())
            .to_json_rpc_error()
            .code,
        MCP_JSONRPC_INVALID_REQUEST
    );
    assert_eq!(
        McpError::Unsupported("bad".to_string())
            .to_json_rpc_error()
            .code,
        MCP_JSONRPC_INVALID_REQUEST
    );
    assert_eq!(
        McpError::NotConnected("missing".to_string())
            .to_json_rpc_error()
            .code,
        MCP_JSONRPC_METHOD_NOT_FOUND
    );
    assert_eq!(
        McpError::Transport("closed".to_string())
            .to_json_rpc_error()
            .code,
        MCP_JSONRPC_INTERNAL_ERROR
    );
    assert_eq!(
        McpError::Protocol("bad frame".to_string())
            .to_json_rpc_error()
            .code,
        MCP_JSONRPC_PARSE_ERROR
    );
}
