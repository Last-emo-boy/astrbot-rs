use serde_json::json;

use crate::schema::{
    AnthropicToolSchemaSerializer, GeminiToolSchemaSerializer, OpenAiToolSchemaSerializer,
};
use crate::{
    CommandDescriptor, CommandPermission, InternalToolProviderCatalog,
    InternalToolProviderDescriptor, InternalToolRegistration, ToolActivationPolicy,
    ToolCallReferencePayload, ToolCatalog, ToolDescriptor, ToolReferenceExtractor,
    ToolSchemaSerializer, ToolSource, ToolSourceMetadata, detect_command_conflicts,
    detect_tool_conflicts,
};

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

#[test]
fn catalog_replaces_tools_and_applies_activation_policy_without_registry_mutation() {
    let mut catalog = ToolCatalog::new();
    catalog.add_tool(ToolDescriptor::new("weather").with_source(ToolSource::Plugin));
    catalog.add_tool(
        weather_tool()
            .with_description("Updated weather")
            .with_source(ToolSource::Mcp),
    );
    catalog.add_tool(ToolDescriptor::new("disabled").inactive());

    let active = catalog.active_tools(&ToolActivationPolicy::new().rename("weather", "forecast"));

    assert_eq!(catalog.tools().len(), 2);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "forecast");
    assert_eq!(active[0].source, ToolSource::Mcp);
}

#[test]
fn openai_anthropic_and_gemini_serializers_are_provider_specific() {
    let tools = vec![weather_tool()];

    let openai = OpenAiToolSchemaSerializer::default().serialize_tools(&tools);
    let anthropic = AnthropicToolSchemaSerializer.serialize_tools(&tools);
    let gemini = GeminiToolSchemaSerializer.serialize_tools(&tools);

    assert_eq!(openai[0]["type"], "function");
    assert_eq!(openai[0]["function"]["name"], "weather");
    assert_eq!(
        openai[0]["function"]["parameters"]["required"],
        json!(["city"])
    );

    assert_eq!(anthropic[0]["name"], "weather");
    assert_eq!(
        anthropic[0]["input_schema"]["properties"]["city"]["type"],
        "string"
    );

    assert_eq!(gemini["function_declarations"][0]["name"], "weather");
    assert_eq!(
        gemini["function_declarations"][0]["parameters"]["properties"]["city"]["type"],
        "string"
    );
}

#[test]
fn command_descriptor_composes_parent_aliases_and_permissions() {
    let command = CommandDescriptor::new("plugin_mod_handler", "plugin", "forecast")
        .with_parent_signature("weather")
        .with_alias("today")
        .with_permission(CommandPermission::Admin);

    assert_eq!(command.effective_command(), "weather forecast");
    assert_eq!(command.effective_aliases(), vec!["weather today"]);
    assert_eq!(command.permission, CommandPermission::Admin);
}

#[test]
fn conflict_detection_reports_enabled_command_and_tool_collisions() {
    let tool_conflicts = detect_tool_conflicts(&[
        ToolDescriptor::new("weather").with_source(ToolSource::Plugin),
        ToolDescriptor::new("weather").with_source(ToolSource::Mcp),
        ToolDescriptor::new("weather")
            .with_source(ToolSource::Internal)
            .inactive(),
    ]);

    let command_conflicts = detect_command_conflicts(&[
        CommandDescriptor::new("a", "plugin-a", "weather"),
        CommandDescriptor::new("b", "plugin-b", "forecast").with_alias("weather"),
        CommandDescriptor::new("c", "plugin-c", "weather").disabled(),
    ]);

    assert_eq!(tool_conflicts.len(), 1);
    assert_eq!(tool_conflicts[0].tool_name, "weather");
    assert_eq!(command_conflicts.len(), 1);
    assert_eq!(command_conflicts[0].command, "weather");
    assert_eq!(command_conflicts[0].handlers, vec!["a", "b"]);
}

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

#[test]
fn internal_provider_catalog_emits_registration_descriptors() {
    let catalog = InternalToolProviderCatalog::new(vec![
        InternalToolProviderDescriptor::new("knowledge_base", "astrbot.core.tools.kb_query")
            .with_registration(InternalToolRegistration::new(
                "knowledge_base",
                "astr_kb_search",
                "Query knowledge base",
                json!({"type": "object"}),
            )),
    ]);
    let mut tool_catalog = ToolCatalog::new();

    catalog.extend_tool_catalog(&mut tool_catalog);

    let tool = tool_catalog
        .tool("astr_kb_search")
        .expect("internal tool should be registered");
    assert_eq!(tool.source, ToolSource::Internal);
    assert_eq!(tool.source.provider_id.as_deref(), Some("knowledge_base"));
    assert!(!tool.source.allows_user_toggle());
}

#[test]
fn tool_reference_extractor_matches_only_used_web_search_refs() {
    let result = json!({
        "results": [
            {
                "title": "Rust",
                "url": "https://www.rust-lang.org",
                "snippet": "Rust language",
                "index": "abcd.1"
            },
            {
                "title": "Ignored",
                "url": "https://example.test/ignored",
                "snippet": "Unused",
                "index": "abcd.2"
            }
        ]
    })
    .to_string();
    let extractor = ToolReferenceExtractor::default().with_favicon(
        "https://www.rust-lang.org",
        "https://www.rust-lang.org/favicon.ico",
    );

    let refs = extractor.extract_from_tool_calls(
        "See <ref>abcd.1</ref>, repeat <ref>abcd.1</ref>, missing <ref>none</ref>.",
        &[ToolCallReferencePayload::new("web_search_tavily", result)],
    );

    assert_eq!(refs.used.len(), 1);
    assert_eq!(refs.used[0].index, "abcd.1");
    assert_eq!(
        refs.used[0].url.as_deref(),
        Some("https://www.rust-lang.org")
    );
    assert_eq!(
        refs.used[0].favicon.as_deref(),
        Some("https://www.rust-lang.org/favicon.ico")
    );
}

#[test]
fn tool_reference_extractor_ignores_unsupported_tools_and_invalid_json() {
    let extractor = ToolReferenceExtractor::default();

    let refs = extractor.extract_from_tool_calls(
        "See <ref>abcd.1</ref>.",
        &[
            ToolCallReferencePayload::new("web_search", "{\"results\": []}"),
            ToolCallReferencePayload::new("web_search_bocha", "{not-json"),
        ],
    );

    assert!(refs.is_empty());
}
