#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use astrbot_plugin_sdk::{builtin_commands, plugin_main, CommandHandlerResult, IncomingCommand, Plugin, PluginInitInfo};

#[derive(Default)]
pub struct BuiltinSid;

impl Plugin for BuiltinSid {
    fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
        vec!["sid"]
    }

    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        builtin_commands::sid(command, &builtin_commands::BuiltinContext::new(&["admin"]))
    }
}

plugin_main!(BuiltinSid);
