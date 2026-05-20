#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use astrbot_plugin_sdk::{builtin_commands, plugin_main, CommandHandlerResult, IncomingCommand, Plugin, PluginInitInfo};

#[derive(Default)]
pub struct BuiltinSetUnset;

impl Plugin for BuiltinSetUnset {
    fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
        vec!["set", "unset", "setunset"]
    }

    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        match command.keyword.as_str() {
            "set" => builtin_commands::set_variable(command),
            "unset" => builtin_commands::unset_variable(command),
            "setunset" => {
                let args = builtin_commands::split_args(&command.argument);
                match args.first().map(alloc::string::String::as_str) {
                    Some("set") => {
                        let forwarded = IncomingCommand {
                            session_id: command.session_id.clone(),
                            sender_id: command.sender_id.clone(),
                            keyword: "set".into(),
                            argument: args[1..].join(" "),
                        };
                        builtin_commands::set_variable(&forwarded)
                    }
                    Some("unset") => {
                        let forwarded = IncomingCommand {
                            session_id: command.session_id.clone(),
                            sender_id: command.sender_id.clone(),
                            keyword: "unset".into(),
                            argument: args[1..].join(" "),
                        };
                        builtin_commands::unset_variable(&forwarded)
                    }
                    _ => builtin_commands::reply_text(command, "用法：/setunset set <key> <value> 或 /setunset unset <key>"),
                }
            }
            _ => CommandHandlerResult::no_op(),
        }
    }
}

plugin_main!(BuiltinSetUnset);
