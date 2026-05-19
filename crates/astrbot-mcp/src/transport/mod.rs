use std::{collections::BTreeMap, time::Duration};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{McpError, McpReconnectPolicy, McpResult, McpServerConfig, McpTransport};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpProcessCommand {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl McpProcessCommand {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            env: BTreeMap::new(),
        }
    }

    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        let arg = arg.into();
        if !arg.trim().is_empty() {
            self.args.push(arg);
        }
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpProcessState {
    #[default]
    NotStarted,
    Starting,
    Running,
    Exited,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpProcessSupervisorPlan {
    pub server_name: String,
    pub command: McpProcessCommand,
    pub state: McpProcessState,
    pub restart_on_exit: bool,
}

impl McpProcessSupervisorPlan {
    pub fn from_server_config(
        server_name: impl Into<String>,
        config: &McpServerConfig,
    ) -> McpResult<Option<Self>> {
        if config.transport != McpTransport::Stdio {
            return Ok(None);
        }
        let command = config.command.as_ref().ok_or_else(|| {
            McpError::InvalidConfig("stdio MCP server requires command".to_string())
        })?;
        Ok(Some(Self {
            server_name: server_name.into(),
            command: McpProcessCommand {
                command: command.clone(),
                args: config.args.clone(),
                env: BTreeMap::new(),
            },
            state: McpProcessState::NotStarted,
            restart_on_exit: true,
        }))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpTransportEndpoint {
    Stdio { command: McpProcessCommand },
    Sse { url: String },
    StreamableHttp { url: String },
}

impl McpTransportEndpoint {
    pub fn from_server_config(config: &McpServerConfig) -> McpResult<Self> {
        match config.transport {
            McpTransport::Stdio => {
                let command = config.command.as_ref().ok_or_else(|| {
                    McpError::InvalidConfig("stdio MCP server requires command".to_string())
                })?;
                Ok(Self::Stdio {
                    command: McpProcessCommand {
                        command: command.clone(),
                        args: config.args.clone(),
                        env: BTreeMap::new(),
                    },
                })
            }
            McpTransport::Sse => Ok(Self::Sse {
                url: config.url.clone().ok_or_else(|| {
                    McpError::InvalidConfig("SSE MCP server requires url".to_string())
                })?,
            }),
            McpTransport::StreamableHttp => Ok(Self::StreamableHttp {
                url: config.url.clone().ok_or_else(|| {
                    McpError::InvalidConfig("streamable HTTP MCP server requires url".to_string())
                })?,
            }),
        }
    }
}

#[async_trait]
pub trait McpTransportRuntime: Send + Sync {
    async fn connect(&self, endpoint: McpTransportEndpoint) -> McpResult<McpTransportSession>;

    async fn send(&self, session: &McpTransportSession, frame: McpJsonRpcFrame) -> McpResult<()>;

    async fn request(
        &self,
        session: &McpTransportSession,
        frame: McpJsonRpcFrame,
        read_timeout: Duration,
    ) -> McpResult<McpJsonRpcFrame>;

    async fn close(&self, session: McpTransportSession) -> McpResult<()>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTransportSession {
    pub session_id: String,
    pub endpoint: McpTransportEndpoint,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpJsonRpcFrame {
    pub value: Value,
}

impl McpJsonRpcFrame {
    pub fn parse(line: &str) -> McpResult<Option<Self>> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            return Ok(None);
        };
        let Some(object) = value.as_object() else {
            return Ok(None);
        };
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Ok(None);
        }
        Ok(Some(Self { value }))
    }

    pub fn method(&self) -> Option<&str> {
        self.value.get("method").and_then(Value::as_str)
    }

    pub fn id(&self) -> Option<&Value> {
        self.value.get("id")
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct McpStdoutParseReport {
    pub frames: Vec<McpJsonRpcFrame>,
    pub ignored_lines: Vec<String>,
}

pub struct McpStdoutJsonRpcParser;

impl McpStdoutJsonRpcParser {
    pub fn parse_lines<'a>(
        lines: impl IntoIterator<Item = &'a str>,
    ) -> McpResult<McpStdoutParseReport> {
        let mut report = McpStdoutParseReport::default();
        for line in lines {
            match McpJsonRpcFrame::parse(line)? {
                Some(frame) => report.frames.push(frame),
                None if !line.trim().is_empty() => report.ignored_lines.push(line.to_string()),
                None => {}
            }
        }
        Ok(report)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpReconnectDecision {
    pub attempt: u32,
    pub delay_ms: Option<u64>,
}

impl McpReconnectDecision {
    pub fn should_retry(&self) -> bool {
        self.delay_ms.is_some()
    }
}

pub fn mcp_reconnect_decision(policy: &McpReconnectPolicy, attempt: u32) -> McpReconnectDecision {
    if attempt >= policy.max_attempts {
        return McpReconnectDecision {
            attempt,
            delay_ms: None,
        };
    }
    let multiplier = 2_u64.saturating_pow(attempt);
    let delay = policy
        .backoff_initial_ms
        .saturating_mul(multiplier)
        .min(policy.backoff_max_ms);
    McpReconnectDecision {
        attempt,
        delay_ms: Some(delay),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        McpJsonRpcFrame, McpProcessSupervisorPlan, McpStdoutJsonRpcParser, McpTransportEndpoint,
        mcp_reconnect_decision,
    };
    use crate::{McpReconnectPolicy, McpServerConfig};

    #[test]
    fn stdout_parser_filters_noisy_lines_before_json_rpc_frames() {
        let report = McpStdoutJsonRpcParser::parse_lines([
            "server started",
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
            r#"{"level":"info","message":"ignored"}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#,
        ])
        .expect("stdout should parse");

        assert_eq!(report.frames.len(), 2);
        assert_eq!(report.frames[0].id(), Some(&json!(1)));
        assert_eq!(
            report.frames[1].method(),
            Some("notifications/tools/list_changed")
        );
        assert_eq!(report.ignored_lines.len(), 2);
    }

    #[test]
    fn transport_endpoint_and_process_plan_come_from_server_config() {
        let config = McpServerConfig::stdio("node").with_arg("server.js");

        let endpoint =
            McpTransportEndpoint::from_server_config(&config).expect("endpoint should build");
        let plan = McpProcessSupervisorPlan::from_server_config("docs", &config)
            .expect("plan should build")
            .expect("stdio should create process plan");

        assert!(matches!(endpoint, McpTransportEndpoint::Stdio { .. }));
        assert_eq!(plan.command.command, "node");
        assert_eq!(plan.command.args, vec!["server.js"]);
    }

    #[test]
    fn reconnect_decision_caps_exponential_backoff() {
        let policy = McpReconnectPolicy {
            max_attempts: 3,
            backoff_initial_ms: 100,
            backoff_max_ms: 250,
        };

        assert_eq!(mcp_reconnect_decision(&policy, 0).delay_ms, Some(100));
        assert_eq!(mcp_reconnect_decision(&policy, 2).delay_ms, Some(250));
        assert!(!mcp_reconnect_decision(&policy, 3).should_retry());
    }

    #[test]
    fn json_rpc_frame_ignores_valid_json_that_is_not_mcp_protocol() {
        assert_eq!(
            McpJsonRpcFrame::parse(r#"{"message":"hello"}"#).expect("json should parse"),
            None
        );
    }
}
