use crate::{build_mcp_prompt_tool_names, build_mcp_resource_tool_names};

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
