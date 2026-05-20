#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use astrbot_plugin_sdk::{builtin_commands, plugin_main, CommandHandlerResult, IncomingCommand, Plugin, PluginInitInfo};

#[derive(Default)]
pub struct BuiltinHelp;

impl Plugin for BuiltinHelp {
    fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
        vec!["help"]
    }

    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        builtin_commands::help(command)
    }
}

plugin_main!(BuiltinHelp);
