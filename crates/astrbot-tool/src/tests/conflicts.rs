use crate::{
    CommandDescriptor, ToolDescriptor, ToolSource, detect_command_conflicts, detect_tool_conflicts,
};

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
