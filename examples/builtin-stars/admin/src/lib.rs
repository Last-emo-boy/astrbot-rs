#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use astrbot_plugin_sdk::{builtin_commands, plugin_main, CommandHandlerResult, IncomingCommand, Plugin, PluginInitInfo};

#[derive(Default)]
pub struct BuiltinAdmin;

impl Plugin for BuiltinAdmin {
    fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
        vec!["admin", "op", "deop", "wl", "dwl"]
    }

    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        builtin_commands::admin(command, &builtin_commands::BuiltinContext::new(&["admin"]))
    }
}

plugin_main!(BuiltinAdmin);
