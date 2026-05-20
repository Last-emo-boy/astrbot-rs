#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use astrbot_plugin_sdk::{builtin_commands, plugin_main, CommandHandlerResult, IncomingCommand, Plugin, PluginInitInfo};

#[derive(Default)]
pub struct BuiltinSessionController;

impl Plugin for BuiltinSessionController {
    fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
        vec!["sleep", "wake", "rate", "session"]
    }

    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        builtin_commands::session_controller(command)
    }
}

plugin_main!(BuiltinSessionController);
