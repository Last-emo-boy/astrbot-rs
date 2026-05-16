use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::types::{McpError, McpJsonObject, McpResult, McpServerName};

pub const DEFAULT_MCP_ELICITATION_TIMEOUT_SECONDS: u64 = 300;
pub const DEFAULT_MCP_SESSION_READ_TIMEOUT_SECONDS: u64 = 60;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    #[serde(default)]
    pub mcp_servers: McpServerConfigs,
}

impl McpConfig {
    pub fn normalize(self) -> Self {
        let servers = self
            .mcp_servers
            .0
            .into_iter()
            .map(|(name, config)| (name, config.normalize()))
            .collect();
        Self {
            mcp_servers: McpServerConfigs(servers),
        }
    }

    pub fn active_servers(&self) -> impl Iterator<Item = (&String, &McpServerConfig)> {
        self.mcp_servers
            .0
            .iter()
            .filter(|(_, config)| config.active)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpServerConfigs(pub BTreeMap<String, McpServerConfig>);

impl McpServerConfigs {
    pub fn insert(
        &mut self,
        name: impl Into<String>,
        config: McpServerConfig,
    ) -> McpResult<McpServerName> {
        let name = McpServerName::new(name)?;
        self.0.insert(name.as_str().to_string(), config);
        Ok(name)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    #[default]
    Stdio,
    Sse,
    StreamableHttp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    #[serde(default = "default_active")]
    pub active: bool,
    #[serde(default)]
    pub transport: McpTransport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "McpJsonObject::is_empty")]
    pub headers: McpJsonObject,
    #[serde(default = "default_session_read_timeout_seconds")]
    pub session_read_timeout_seconds: u64,
    #[serde(default)]
    pub client_capabilities: McpClientCapabilities,
}

impl McpServerConfig {
    pub fn stdio(command: impl Into<String>) -> Self {
        Self {
            command: Some(command.into()),
            ..Self::default()
        }
    }

    pub fn sse(url: impl Into<String>) -> Self {
        Self {
            transport: McpTransport::Sse,
            url: Some(url.into()),
            ..Self::default()
        }
    }

    pub fn streamable_http(url: impl Into<String>) -> Self {
        Self {
            transport: McpTransport::StreamableHttp,
            url: Some(url.into()),
            ..Self::default()
        }
    }

    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        let arg = arg.into();
        if !arg.trim().is_empty() {
            self.args.push(arg);
        }
        self
    }

    pub fn with_client_capabilities(mut self, capabilities: McpClientCapabilities) -> Self {
        self.client_capabilities = capabilities;
        self
    }

    pub fn normalize(mut self) -> Self {
        if self.url.as_ref().is_some_and(|url| !url.trim().is_empty())
            && self.transport == McpTransport::Stdio
        {
            self.transport = McpTransport::Sse;
        }
        self.command = self
            .command
            .map(|command| command.trim().to_string())
            .filter(|command| !command.is_empty());
        self.url = self
            .url
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty());
        self.args.retain(|arg| !arg.trim().is_empty());
        if self.session_read_timeout_seconds == 0 {
            self.session_read_timeout_seconds = DEFAULT_MCP_SESSION_READ_TIMEOUT_SECONDS;
        }
        self.client_capabilities = self.client_capabilities.normalize();
        self
    }

    pub fn validate(&self) -> McpResult<()> {
        match self.transport {
            McpTransport::Stdio if self.command.is_none() => Err(McpError::InvalidConfig(
                "stdio MCP server requires a command".to_string(),
            )),
            McpTransport::Sse | McpTransport::StreamableHttp if self.url.is_none() => Err(
                McpError::InvalidConfig("HTTP MCP server requires a url".to_string()),
            ),
            _ => Ok(()),
        }
    }

    pub fn session_read_timeout(&self) -> Duration {
        Duration::from_secs(self.session_read_timeout_seconds)
    }
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            active: true,
            transport: McpTransport::Stdio,
            command: None,
            args: Vec::new(),
            url: None,
            headers: McpJsonObject::new(),
            session_read_timeout_seconds: DEFAULT_MCP_SESSION_READ_TIMEOUT_SECONDS,
            client_capabilities: McpClientCapabilities::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientCapabilities {
    pub elicitation: McpElicitationCapabilityConfig,
    pub sampling: McpSamplingCapabilityConfig,
    pub roots: crate::roots::McpRootsCapabilityConfig,
}

impl McpClientCapabilities {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub fn normalize(mut self) -> Self {
        if self.elicitation.timeout_seconds == 0 {
            self.elicitation.timeout_seconds = DEFAULT_MCP_ELICITATION_TIMEOUT_SECONDS;
        }
        self.roots.paths.retain(|path| !path.trim().is_empty());
        self
    }

    pub fn supports_interactive_requests(&self) -> bool {
        self.elicitation.enabled || self.sampling.enabled
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpElicitationCapabilityConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_elicitation_timeout_seconds")]
    pub timeout_seconds: u64,
}

impl Default for McpElicitationCapabilityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_seconds: DEFAULT_MCP_ELICITATION_TIMEOUT_SECONDS,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSamplingCapabilityConfig {
    #[serde(default)]
    pub enabled: bool,
}

fn default_active() -> bool {
    true
}

fn default_elicitation_timeout_seconds() -> u64 {
    DEFAULT_MCP_ELICITATION_TIMEOUT_SECONDS
}

fn default_session_read_timeout_seconds() -> u64 {
    DEFAULT_MCP_SESSION_READ_TIMEOUT_SECONDS
}
