use serde_json::json;

use crate::schema::{
    AnthropicToolSchemaSerializer, GeminiToolSchemaSerializer, OpenAiToolSchemaSerializer,
};
use crate::{
    CommandDescriptor, CommandPermission, ToolActivationPolicy, ToolCatalog, ToolDescriptor,
    ToolSchemaSerializer, ToolSource, detect_command_conflicts, detect_tool_conflicts,
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
