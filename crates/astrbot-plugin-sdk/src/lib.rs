//! AstrBot WASM Plugin SDK.
//!
//! This crate gives you everything you need to write a plugin for AstrBot's
//! WASM sandbox in Rust. The shape of a plugin is small: declare a struct
//! that implements [`Plugin`], then hand it to [`plugin_main`] in a single
//! macro invocation.
//!
//! ```ignore
//! use astrbot_plugin_sdk::{plugin_main, CommandHandlerResult, IncomingCommand, OutboundMessage, Plugin, PluginInitInfo};
//!
//! struct Echo;
//!
//! impl Plugin for Echo {
//!     fn on_init(&self, info: &PluginInitInfo) -> Vec<&'static str> {
//!         vec!["echo"]
//!     }
//!
//!     fn on_command(&self, cmd: &IncomingCommand) -> CommandHandlerResult {
//!         CommandHandlerResult::reply(OutboundMessage {
//!             session_id: cmd.session_id.clone(),
//!             text: cmd.argument.clone(),
//!         })
//!     }
//! }
//!
//! plugin_main!(Echo);
//! ```
//!
//! At runtime the macro generates the required `extern "C"` exports and
//! delegates them through the trait methods. Allocation is handled internally
//! via a bump allocator backed by `Vec<u8>` leaks — the host calls
//! `astrbot_free` to return memory, but in practice plugins rarely free in
//! between dispatch calls.

#![cfg_attr(target_arch = "wasm32", no_std)]
#![cfg_attr(target_arch = "wasm32", allow(internal_features))]

#[cfg(not(target_arch = "wasm32"))]
extern crate std;
extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

pub use serde::{Deserialize, Serialize};
pub use serde_json;

/// ABI major version this SDK targets. Must match
/// `astrbot_plugin::wasm::abi::ABI_VERSION_MAJOR` on the host.
pub const ABI_VERSION_MAJOR: i32 = 1;
/// ABI minor version this SDK targets.
pub const ABI_VERSION_MINOR: i32 = 0;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginInitInfo {
    pub plugin_id: String,
    pub host_abi_major: i32,
    pub host_abi_minor: i32,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginInitResponse {
    pub plugin_abi_major: i32,
    pub plugin_abi_minor: i32,
    #[serde(default)]
    pub commands: Vec<CommandRegistration>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandRegistration {
    pub keyword: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginEvent {
    Message(IncomingMessage),
    Command(IncomingCommand),
    Ping,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncomingMessage {
    pub session_id: String,
    pub sender_id: String,
    #[serde(default)]
    pub sender_name: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncomingCommand {
    pub session_id: String,
    pub sender_id: String,
    pub keyword: String,
    pub argument: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginResponse {
    Replies { messages: Vec<OutboundMessage> },
    Pong,
    NoOp,
    Error { message: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutboundMessage {
    pub session_id: String,
    pub text: String,
}

/// Lightweight reply container returned from [`Plugin::on_command`].
pub enum CommandHandlerResult {
    Replies(Vec<OutboundMessage>),
    NoOp,
}

impl CommandHandlerResult {
    pub fn reply(message: OutboundMessage) -> Self {
        let mut v = Vec::new();
        v.push(message);
        CommandHandlerResult::Replies(v)
    }

    pub fn no_op() -> Self {
        CommandHandlerResult::NoOp
    }

    pub fn into_response(self) -> PluginResponse {
        match self {
            CommandHandlerResult::Replies(messages) => PluginResponse::Replies { messages },
            CommandHandlerResult::NoOp => PluginResponse::NoOp,
        }
    }
}

/// Implement this trait on the plugin's main struct and hand it to
/// [`plugin_main`].
pub trait Plugin {
    /// Called once after the host instantiates the plugin. The returned
    /// keywords are registered as commands.
    fn on_init(&self, info: &PluginInitInfo) -> Vec<&'static str>;

    /// Called for every inbound command matching one of the keywords
    /// returned by [`Plugin::on_init`]. The default implementation echoes
    /// the argument.
    fn on_command(&self, command: &IncomingCommand) -> CommandHandlerResult {
        CommandHandlerResult::reply(OutboundMessage {
            session_id: command.session_id.clone(),
            text: command.argument.clone(),
        })
    }

    /// Called for every inbound plain message. Default = NoOp.
    fn on_message(&self, _message: &IncomingMessage) -> CommandHandlerResult {
        CommandHandlerResult::NoOp
    }
}

/// Build a successful init response with the given command keywords.
pub fn init_response(keywords: &[&str]) -> PluginInitResponse {
    let mut commands = Vec::new();
    for keyword in keywords {
        let mut keyword_string = String::new();
        keyword_string.push_str(keyword);
        commands.push(CommandRegistration {
            keyword: keyword_string,
            description: None,
        });
    }
    PluginInitResponse {
        plugin_abi_major: ABI_VERSION_MAJOR,
        plugin_abi_minor: ABI_VERSION_MINOR,
        commands,
    }
}

/// Pack a `(ptr, len)` pair into the `u64` the ABI expects from guest
/// returns.
#[inline]
pub fn pack_ptr_len(ptr: u32, len: u32) -> u64 {
    ((ptr as u64) << 32) | (len as u64)
}

#[inline]
pub fn unpack_ptr_len(value: u64) -> (u32, u32) {
    ((value >> 32) as u32, (value & 0xFFFF_FFFF) as u32)
}

/// Generates the `extern "C"` exports the host expects, dispatching them
/// through the trait methods on the provided plugin type.
///
/// Usage: `plugin_main!(MyPlugin);` where `MyPlugin: Plugin + Default`.
#[macro_export]
macro_rules! plugin_main {
    ($plugin:ty) => {
        const _: fn() = || {
            fn assert_plugin<T: $crate::Plugin + Default>() {}
            assert_plugin::<$plugin>();
        };

        /// Bump allocator: hands out fresh `Vec<u8>` regions and forgets
        /// about them. `astrbot_free` is the inverse — callers pass the
        /// `(ptr, len)` they got from `astrbot_alloc`.
        #[no_mangle]
        pub extern "C" fn astrbot_alloc(len: i32) -> i32 {
            if len <= 0 {
                return 0;
            }
            let mut buf: ::alloc::vec::Vec<u8> = ::alloc::vec::Vec::with_capacity(len as usize);
            unsafe { buf.set_len(len as usize) };
            let ptr = buf.as_mut_ptr();
            ::core::mem::forget(buf);
            ptr as i32
        }

        #[no_mangle]
        pub extern "C" fn astrbot_free(ptr: i32, len: i32) {
            if ptr <= 0 || len <= 0 {
                return;
            }
            unsafe {
                let _ = ::alloc::vec::Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize);
            }
        }

        #[no_mangle]
        pub extern "C" fn astrbot_abi_version() -> i32 {
            $crate::ABI_VERSION_MAJOR
        }

        #[no_mangle]
        pub extern "C" fn astrbot_init(ptr: i32, len: i32) -> i64 {
            let request_bytes = unsafe {
                ::core::slice::from_raw_parts(ptr as *const u8, len as usize)
            };
            let info: $crate::PluginInitInfo = match $crate::serde_json::from_slice(request_bytes) {
                Ok(v) => v,
                Err(_) => return 0,
            };
            let plugin = <$plugin>::default();
            let keywords = $crate::Plugin::on_init(&plugin, &info);
            let response = $crate::init_response(&keywords);
            let bytes = match $crate::serde_json::to_vec(&response) {
                Ok(v) => v,
                Err(_) => return 0,
            };
            $crate::publish_bytes(bytes)
        }

        #[no_mangle]
        pub extern "C" fn astrbot_dispatch(ptr: i32, len: i32) -> i64 {
            let event_bytes = unsafe {
                ::core::slice::from_raw_parts(ptr as *const u8, len as usize)
            };
            let event: $crate::PluginEvent = match $crate::serde_json::from_slice(event_bytes) {
                Ok(v) => v,
                Err(_) => return 0,
            };
            let plugin = <$plugin>::default();
            let response = match event {
                $crate::PluginEvent::Ping => $crate::PluginResponse::Pong,
                $crate::PluginEvent::Command(cmd) => {
                    $crate::Plugin::on_command(&plugin, &cmd).into_response()
                }
                $crate::PluginEvent::Message(msg) => {
                    $crate::Plugin::on_message(&plugin, &msg).into_response()
                }
            };
            let bytes = match $crate::serde_json::to_vec(&response) {
                Ok(v) => v,
                Err(_) => return 0,
            };
            $crate::publish_bytes(bytes)
        }
    };
}

/// Move `bytes` into a heap allocation owned by the host. The pointer and
/// length are packed into the ABI's `u64` shape. Called by the generated
/// `astrbot_init` / `astrbot_dispatch` shims.
#[doc(hidden)]
pub fn publish_bytes(bytes: Vec<u8>) -> i64 {
    let len = bytes.len();
    let mut owned = bytes;
    owned.shrink_to_fit();
    let ptr = owned.as_mut_ptr();
    core::mem::forget(owned);
    pack_ptr_len(ptr as u32, len as u32) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Echo;

    impl Plugin for Echo {
        fn on_init(&self, _info: &PluginInitInfo) -> Vec<&'static str> {
            let mut v = Vec::new();
            v.push("echo");
            v
        }
    }

    #[test]
    fn init_response_carries_abi_and_commands() {
        let response = init_response(&["echo", "ping"]);
        assert_eq!(response.plugin_abi_major, ABI_VERSION_MAJOR);
        assert_eq!(response.plugin_abi_minor, ABI_VERSION_MINOR);
        assert_eq!(response.commands.len(), 2);
        assert_eq!(response.commands[0].keyword, "echo");
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let ptr = 0xDEAD_BEEF_u32;
        let len = 0x42_u32;
        let packed = pack_ptr_len(ptr, len);
        assert_eq!(unpack_ptr_len(packed), (ptr, len));
    }

    #[test]
    fn command_handler_no_op_serialises() {
        let response = CommandHandlerResult::no_op().into_response();
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"type\":\"no_op\""));
    }

    #[test]
    fn command_handler_replies_carries_payload() {
        let response = CommandHandlerResult::reply(OutboundMessage {
            session_id: String::from("s1"),
            text: String::from("hi"),
        })
        .into_response();
        match response {
            PluginResponse::Replies { messages } => {
                assert_eq!(messages.len(), 1);
                assert_eq!(messages[0].text, "hi");
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn default_on_message_returns_no_op() {
        let echo = Echo::default();
        let message = IncomingMessage {
            session_id: String::from("s1"),
            sender_id: String::from("u1"),
            sender_name: None,
            text: String::from("hello"),
        };
        match echo.on_message(&message) {
            CommandHandlerResult::NoOp => {}
            CommandHandlerResult::Replies(_) => panic!("expected NoOp default"),
        }
    }
}
