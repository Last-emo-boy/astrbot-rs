#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use astrbot_plugin_sdk::{builtin_commands, plugin_main, CommandHandlerResult, IncomingCommand, Plugin, PluginInitInfo};

#[derive(Default)]
pub struct BuiltinProvider;

impl Plugin for BuiltinProvider {
    fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
        vec!["provider"]
    }

    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        builtin_commands::provider(command, &builtin_commands::BuiltinContext::new(&["admin"]))
    }
}

plugin_main!(BuiltinProvider);
