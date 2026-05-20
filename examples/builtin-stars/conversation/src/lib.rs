#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use astrbot_plugin_sdk::{builtin_commands, plugin_main, CommandHandlerResult, IncomingCommand, Plugin, PluginInitInfo};

#[derive(Default)]
pub struct BuiltinConversation;

impl Plugin for BuiltinConversation {
    fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
        vec!["reset", "stop", "history", "ls", "new", "groupnew", "switch", "rename", "del", "key"]
    }

    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        builtin_commands::conversation(command)
    }
}

plugin_main!(BuiltinConversation);
