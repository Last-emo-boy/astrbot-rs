//! Echo plugin — minimal demonstration of the AstrBot WASM SDK.
//!
//! Build with `cargo build --target wasm32-wasip1 --release` from this
//! directory. The resulting `target/wasm32-wasip1/release/echo_plugin.wasm`
//! is what the host loads after copying it next to `plugin.toml`.

#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use astrbot_plugin_sdk::{
    CommandHandlerResult, IncomingCommand, OutboundMessage, Plugin, PluginInitInfo, plugin_main,
};

#[derive(Default)]
pub struct Echo;

impl Plugin for Echo {
    fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
        let mut keywords: Vec<&'static str> = Vec::new();
        keywords.push("echo");
        keywords
    }

    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        let mut text = String::new();
        if command.argument.is_empty() {
            text.push_str("hello");
        } else {
            text.push_str(&command.argument);
        }
        CommandHandlerResult::reply(OutboundMessage {
            session_id: command.session_id.clone(),
            text,
        })
    }
}

plugin_main!(Echo);
