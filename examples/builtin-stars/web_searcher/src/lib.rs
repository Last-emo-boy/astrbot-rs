#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use astrbot_plugin_sdk::{builtin_commands, plugin_main, CommandHandlerResult, IncomingCommand, Plugin, PluginInitInfo};

#[derive(Default)]
pub struct BuiltinWebSearcher;

impl Plugin for BuiltinWebSearcher {
    fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
        vec!["websearch", "web_search"]
    }

    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        builtin_commands::web_searcher(command)
    }
}

plugin_main!(BuiltinWebSearcher);
