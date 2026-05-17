use crate::{CommandDescriptor, CommandPermission};

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
