#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use astrbot_plugin_sdk::{builtin_commands, plugin_main, CommandHandlerResult, IncomingCommand, Plugin, PluginInitInfo};

#[derive(Default)]
pub struct BuiltinLlm;

impl Plugin for BuiltinLlm {
    fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
        vec!["llm", "model"]
    }

    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        let ctx = builtin_commands::BuiltinContext::new(&["admin"]);
        match command.keyword.as_str() {
            "model" => builtin_commands::model(command, &ctx),
            _ => builtin_commands::llm(command, &ctx),
        }
    }
}

plugin_main!(BuiltinLlm);
