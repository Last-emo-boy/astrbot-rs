use crate::{
    BUILTIN_COMMAND_PLUGIN_NAME, CommandDescriptor, CommandPermission, CommandType,
    builtin_command_descriptors,
};

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
fn builtin_command_catalog_matches_reserved_python_command_surface() {
    let commands = builtin_command_descriptors();
    let names = commands
        .iter()
        .map(|command| command.effective_command())
        .collect::<Vec<_>>();

    assert_eq!(commands.len(), 32);
    for expected in [
        "alter_cmd",
        "dashboard_update",
        "deop",
        "del",
        "dwl",
        "groupnew",
        "help",
        "history",
        "key",
        "llm",
        "ls",
        "model",
        "new",
        "op",
        "persona",
        "plugin",
        "plugin get",
        "plugin help",
        "plugin ls",
        "plugin off",
        "plugin on",
        "provider",
        "rename",
        "reset",
        "set",
        "sid",
        "stop",
        "switch",
        "t2i",
        "tts",
        "unset",
        "wl",
    ] {
        assert!(names.contains(&expected.to_string()), "missing {expected}");
    }

    let plugin = commands
        .iter()
        .find(|command| command.effective_command() == "plugin")
        .expect("plugin group should exist");
    assert_eq!(plugin.command_type, CommandType::Group);
    assert_eq!(plugin.plugin_name, BUILTIN_COMMAND_PLUGIN_NAME);
    assert!(plugin.reserved);

    let plugin_off = commands
        .iter()
        .find(|command| command.effective_command() == "plugin off")
        .expect("plugin off should exist");
    assert_eq!(plugin_off.command_type, CommandType::SubCommand);
    assert_eq!(plugin_off.permission, CommandPermission::Admin);

    let alter = commands
        .iter()
        .find(|command| command.effective_command() == "alter_cmd")
        .expect("alter_cmd should exist");
    assert_eq!(alter.effective_aliases(), vec!["alter"]);
}
