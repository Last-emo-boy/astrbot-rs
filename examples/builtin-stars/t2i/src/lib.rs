#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use astrbot_plugin_sdk::{builtin_commands, plugin_main, CommandHandlerResult, IncomingCommand, Plugin, PluginInitInfo};

#[derive(Default)]
pub struct BuiltinT2i;

impl Plugin for BuiltinT2i {
    fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
        vec!["t2i"]
    }

    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        builtin_commands::t2i(command)
    }
}

plugin_main!(BuiltinT2i);
