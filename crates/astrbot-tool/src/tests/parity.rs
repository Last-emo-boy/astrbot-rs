use serde_json::json;

use crate::schema::{
    AnthropicToolSchemaSerializer, GeminiToolSchemaSerializer, OpenAiToolSchemaSerializer,
};
use crate::{
    CommandDescriptor, CommandPermission, CommandType, ToolActivationPolicy, ToolCatalog,
    ToolDescriptor, ToolSchemaSerializer, ToolSource, ToolSourceMetadata, detect_command_conflicts,
    detect_tool_conflicts,
};

#[test]
fn python_toolset_serializer_fixture_matches_provider_shapes() {
    let empty = ToolDescriptor::new("ping");
    let weather = python_weather_fixture_tool();
    let tools = vec![empty, weather];

    let openai_default = OpenAiToolSchemaSerializer::default().serialize_tools(&tools);
    assert_eq!(
        openai_default[0]["function"]["parameters"]["type"],
        "object"
    );
    assert_eq!(
        openai_default[1]["function"]["parameters"]["properties"]["city"]["type"],
        json!(["string", "null"])
    );

    let openai_omit_empty = OpenAiToolSchemaSerializer {
        omit_empty_parameters: true,
    }
    .serialize_tools(&tools);
    assert!(openai_omit_empty[0]["function"].get("parameters").is_none());
    assert_eq!(
        openai_omit_empty[1]["function"]["parameters"]["required"],
        json!(["city", "count"])
    );

    let anthropic = AnthropicToolSchemaSerializer.serialize_tools(&tools);
    assert_eq!(anthropic[1]["name"], "forecast");
    assert_eq!(
        anthropic[1]["input_schema"]["properties"]["city"]["type"],
        json!(["string", "null"])
    );
    assert_eq!(
        anthropic[1]["input_schema"]["required"],
        json!(["city", "count"])
    );

    let gemini = GeminiToolSchemaSerializer.serialize_tools(&tools);
    let forecast = &gemini["function_declarations"][1];
    assert_eq!(
        forecast["parameters"]["properties"]["city"]["type"],
        "string"
    );
    assert_eq!(
        forecast["parameters"]["properties"]["city"]["nullable"],
        true
    );
    assert!(
        forecast["parameters"]["properties"]["city"]
            .get("default")
            .is_none()
    );
    assert!(
        forecast["parameters"]["properties"]["city"]
            .get("additionalProperties")
            .is_none()
    );
    assert_eq!(
        forecast["parameters"]["properties"]["count"]["format"],
        "int64"
    );
    assert_eq!(
        forecast["parameters"]["properties"]["tags"]["items"]["type"],
        "string"
    );
    assert_eq!(
        forecast["parameters"]["properties"]["mode"]["anyOf"][0]["enum"],
        json!(["fast", "full"])
    );
    assert!(
        forecast["parameters"]["properties"]["mode"]
            .get("default")
            .is_none()
    );
}

#[test]
fn source_activation_and_conflict_fixture_matches_python_management_surface() {
    let background = ToolSourceMetadata::background("plugin.jobs", "Jobs Plugin");
    assert_eq!(background.kind, ToolSource::Background);
    assert_eq!(background.source_label(), "plugin");
    assert_eq!(background.origin(), "plugin");
    assert_eq!(background.origin_name(), "Jobs Plugin");
    assert!(background.allows_user_toggle());

    let mut catalog = ToolCatalog::new();
    catalog.add_tool(ToolDescriptor::new("weather").with_source_metadata(
        ToolSourceMetadata::plugin("plugin.weather", "Weather Plugin"),
    ));
    catalog.add_tool(
        ToolDescriptor::new("astr_kb_search")
            .with_source_metadata(ToolSourceMetadata::internal_provider("kb", "AstrBot")),
    );
    catalog.add_tool(
        ToolDescriptor::new("docs_search").with_source_metadata(ToolSourceMetadata::mcp("docs")),
    );
    catalog.add_tool(
        ToolDescriptor::new("delegate")
            .with_source_metadata(ToolSourceMetadata::subagent("writer")),
    );
    catalog.add_tool(ToolDescriptor::new("long_job").with_source_metadata(background));
    catalog.add_tool(ToolDescriptor::new("inactive").inactive());

    let active = catalog.active_tools(
        &ToolActivationPolicy::new()
            .disable("weather")
            .disable("astr_kb_search")
            .disable("long_job")
            .rename("docs_search", "docs_lookup"),
    );
    let names = active
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["astr_kb_search", "delegate", "docs_lookup"]);

    let conflicts = detect_tool_conflicts(&[
        ToolDescriptor::new("lookup")
            .with_source_metadata(ToolSourceMetadata::plugin("plugin.lookup", "Lookup Plugin")),
        ToolDescriptor::new("lookup").with_source_metadata(ToolSourceMetadata::mcp("lookup-mcp")),
        ToolDescriptor::new("lookup")
            .with_source_metadata(ToolSourceMetadata::internal("AstrBot"))
            .inactive(),
    ]);

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].tool_name, "lookup");
    assert_eq!(conflicts[0].sources, vec!["plugin", "mcp"]);
}

#[test]
fn command_parent_alias_conflict_fixture_matches_python_effective_names() {
    let subcommand = CommandDescriptor::new("plugin.weather.forecast", "Weather", "forecast")
        .with_command_type(CommandType::SubCommand)
        .with_parent_signature("weather")
        .with_alias("today")
        .with_permission(CommandPermission::Admin);
    let group = CommandDescriptor::new("plugin.weather.group", "Weather", "weather")
        .with_command_type(CommandType::Group)
        .with_alias("w");

    assert_eq!(subcommand.effective_command(), "weather forecast");
    assert_eq!(subcommand.effective_aliases(), vec!["weather today"]);
    assert_eq!(subcommand.permission, CommandPermission::Admin);
    assert!(!group.is_executable());

    let conflicts = detect_command_conflicts(&[
        subcommand,
        CommandDescriptor::new("plugin.other.lookup", "Other", "lookup")
            .with_alias("weather today"),
        CommandDescriptor::new("plugin.disabled.forecast", "Other", "weather forecast").disabled(),
    ]);

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].command, "weather today");
    assert_eq!(
        conflicts[0].handlers,
        vec!["plugin.weather.forecast", "plugin.other.lookup"]
    );
}

fn python_weather_fixture_tool() -> ToolDescriptor {
    ToolDescriptor::new("forecast")
        .with_description("Get a weather forecast")
        .with_parameters(json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": ["string", "null"],
                    "description": "City name",
                    "nullable": true,
                    "default": "Shanghai",
                    "additionalProperties": false
                },
                "count": {
                    "type": "integer",
                    "format": "int64",
                    "minimum": 1,
                    "maximum": 5
                },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": ["string", "null"],
                        "description": "Optional tag"
                    }
                },
                "mode": {
                    "anyOf": [
                        { "type": "string", "enum": ["fast", "full"] },
                        { "type": "null" }
                    ],
                    "default": "fast"
                }
            },
            "required": ["city", "count"]
        }))
}
