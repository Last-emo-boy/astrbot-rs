use crate::{ToolActivationPolicy, ToolCatalog, ToolDescriptor, ToolSource, ToolSourceMetadata};

#[test]
fn source_metadata_distinguishes_tool_origins_and_toggle_policy() {
    let internal = ToolSourceMetadata::internal_provider("knowledge_base", "AstrBot");
    let plugin = ToolSourceMetadata::plugin("weather_plugin", "Weather Plugin");
    let mcp = ToolSourceMetadata::mcp("docs-server");
    let subagent = ToolSourceMetadata::subagent("writer");

    assert_eq!(internal.kind, ToolSource::Internal);
    assert_eq!(internal.origin(), "internal");
    assert_eq!(internal.origin_name(), "AstrBot");
    assert_eq!(internal.provider_id.as_deref(), Some("knowledge_base"));
    assert!(!internal.allows_user_toggle());

    assert_eq!(plugin.kind, ToolSource::Plugin);
    assert_eq!(plugin.plugin_id.as_deref(), Some("weather_plugin"));
    assert!(plugin.allows_user_toggle());

    assert_eq!(mcp.kind, ToolSource::Mcp);
    assert_eq!(mcp.mcp_server_name.as_deref(), Some("docs-server"));
    assert_eq!(mcp.origin_name(), "docs-server");

    assert_eq!(subagent.kind, ToolSource::Subagent);
    assert_eq!(subagent.subagent_id.as_deref(), Some("writer"));
    assert_eq!(subagent.origin(), "subagent");
}

#[test]
fn activation_policy_does_not_disable_internal_tools_unless_policy_allows_it() {
    let mut catalog = ToolCatalog::new();
    catalog.add_tool(
        ToolDescriptor::new("builtin")
            .with_source_metadata(ToolSourceMetadata::internal("AstrBot")),
    );
    catalog.add_tool(
        ToolDescriptor::new("optional_builtin")
            .with_source_metadata(ToolSourceMetadata::internal("AstrBot").allow_user_toggle()),
    );
    catalog.add_tool(ToolDescriptor::new("plugin_tool").with_source(ToolSource::Plugin));

    let active = catalog.active_tools(
        &ToolActivationPolicy::new()
            .disable("builtin")
            .disable("optional_builtin")
            .disable("plugin_tool"),
    );
    let names = active
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["builtin"]);
}
