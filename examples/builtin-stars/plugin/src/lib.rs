#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use astrbot_plugin_sdk::{builtin_commands, plugin_main, CommandHandlerResult, IncomingCommand, Plugin, PluginInitInfo};

#[derive(Default)]
pub struct BuiltinPlugin;

impl Plugin for BuiltinPlugin {
    fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
        vec!["plugin"]
    }

    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        builtin_commands::plugin(command, &builtin_commands::BuiltinContext::new(&["admin"]))
    }
}

plugin_main!(BuiltinPlugin);
