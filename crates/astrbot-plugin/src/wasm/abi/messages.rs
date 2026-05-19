//! Wire types for the WASM plugin ABI.
//!
//! All payloads cross the host/guest boundary as UTF-8 JSON. We keep the
//! schemas deliberately narrow — this is the contract every plugin compiles
//! against, so each new field bumps [`crate::wasm::abi::ABI_VERSION_MINOR`].

use serde::{Deserialize, Serialize};

/// Sent from host to guest immediately after instantiation.
///
/// The guest answers with [`PluginInitResponse`] enumerating its registered
/// commands and any startup errors.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginInitRequest {
    /// Plugin id assigned by the manifest.
    pub plugin_id: String,
    /// ABI major version the host implements. Guests refuse to load on
    /// mismatch.
    pub host_abi_major: i32,
    /// ABI minor version the host implements.
    pub host_abi_minor: i32,
    /// Engine-side configuration the guest may surface to the user (e.g.
    /// runtime flags, cron grants).
    #[serde(default)]
    pub config: serde_json::Map<String, serde_json::Value>,
    /// Capabilities the host granted to this instance. The guest cannot
    /// upgrade past what it sees here.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl PluginInitRequest {
    pub fn new(plugin_id: impl Into<String>, host_abi_major: i32, host_abi_minor: i32) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            host_abi_major,
            host_abi_minor,
            config: serde_json::Map::new(),
            capabilities: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PluginInitResponse {
    /// ABI major version the guest was compiled against.
    pub plugin_abi_major: i32,
    /// ABI minor version the guest was compiled against.
    pub plugin_abi_minor: i32,
    /// Commands the guest is offering to route. Filled in at init time so the
    /// host can build a command table.
    #[serde(default)]
    pub commands: Vec<CommandRegistration>,
    /// Optional human-readable status message.
    #[serde(default)]
    pub status: Option<String>,
    /// If non-empty, host treats the plugin as failed and unloads it.
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandRegistration {
    /// Command keyword, e.g. `"echo"`. The host prefixes with its own command
    /// indicator (`/`).
    pub keyword: String,
    /// Human-readable description shown in `/help`.
    #[serde(default)]
    pub description: Option<String>,
}

/// Inbound event delivered to the guest's `astrbot_dispatch`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginEvent {
    /// A user message arrived on a session the plugin is registered for.
    Message(IncomingMessage),
    /// A command keyword the plugin registered matched the inbound chat.
    Command(IncomingCommand),
    /// Host pinged the plugin for liveness; expect a [`PluginResponse::Pong`].
    Ping,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncomingMessage {
    pub session_id: String,
    pub sender_id: String,
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

/// Outbound response from the guest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginResponse {
    /// Send zero or more chat replies. The host fans these into the message
    /// pipeline.
    Replies { messages: Vec<OutboundMessage> },
    /// Acknowledgement for [`PluginEvent::Ping`].
    Pong,
    /// Plugin saw the event but chose not to act.
    NoOp,
    /// Plugin reports an error. The host logs and continues.
    Error { message: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutboundMessage {
    pub session_id: String,
    pub text: String,
}

/// Single log record forwarded from guest to host's tracing subscriber.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogRecord {
    pub level: LogLevel,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_request_roundtrip() {
        let req = PluginInitRequest::new("plugin.echo", 1, 0);
        let json = serde_json::to_string(&req).unwrap();
        let parsed: PluginInitRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, req);
    }

    #[test]
    fn plugin_event_command_roundtrip() {
        let event = PluginEvent::Command(IncomingCommand {
            session_id: "s1".into(),
            sender_id: "u1".into(),
            keyword: "echo".into(),
            argument: "hello".into(),
        });
        let json = serde_json::to_string(&event).unwrap();
        let parsed: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn plugin_response_replies_serialises_as_tagged_enum() {
        let resp = PluginResponse::Replies {
            messages: vec![OutboundMessage {
                session_id: "s1".into(),
                text: "hello".into(),
            }],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"type\":\"replies\""));
        let parsed: PluginResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, resp);
    }
}
