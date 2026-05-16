use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::config::McpServerConfig;
use crate::elicitation::{McpElicitationRequest, McpElicitationResult};
use crate::prompts::{McpGetPromptRequest, McpGetPromptResult, McpPrompt};
use crate::resources::{McpReadResourceRequest, McpReadResourceResult, McpResource};
use crate::roots::{McpRoot, McpRootsRequest};
use crate::sampling::{McpSamplingRequest, McpSamplingResult};
use crate::tools::{McpTool, McpToolCallRequest, McpToolCallResult};
use crate::types::{McpCursor, McpListPage, McpResult, McpServerName};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpClientState {
    #[default]
    Disconnected,
    Connecting,
    Ready,
    Reconnecting,
    Failed,
    ShuttingDown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpReconnectPolicy {
    pub max_attempts: u32,
    pub backoff_initial_ms: u64,
    pub backoff_max_ms: u64,
}

impl Default for McpReconnectPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 2,
            backoff_initial_ms: 1_000,
            backoff_max_ms: 3_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConnectionReport {
    pub server_name: McpServerName,
    pub state: McpClientState,
    pub message: Option<String>,
}

impl McpConnectionReport {
    pub fn ready(server_name: McpServerName) -> Self {
        Self {
            server_name,
            state: McpClientState::Ready,
            message: None,
        }
    }

    pub fn failed(server_name: McpServerName, message: impl Into<String>) -> Self {
        Self {
            server_name,
            state: McpClientState::Failed,
            message: Some(message.into()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientSnapshot {
    pub state: McpClientState,
    pub tools: Vec<McpTool>,
    pub resources: Vec<McpResource>,
    pub prompts: Vec<McpPrompt>,
    pub roots: Vec<McpRoot>,
    pub server_errors: Vec<String>,
}

#[async_trait]
pub trait McpClientLifecycle: Send + Sync {
    async fn connect(
        &self,
        server_name: McpServerName,
        config: McpServerConfig,
    ) -> McpResult<McpConnectionReport>;

    async fn reconnect(&self, policy: McpReconnectPolicy) -> McpResult<McpConnectionReport>;

    async fn shutdown(&self) -> McpResult<()>;

    fn snapshot(&self) -> McpClientSnapshot;
}

#[async_trait]
pub trait McpClientBoundary: McpClientLifecycle {
    async fn list_tools(&self, cursor: Option<McpCursor>) -> McpResult<McpListPage<McpTool>>;

    async fn call_tool(&self, request: McpToolCallRequest) -> McpResult<McpToolCallResult>;

    async fn list_resources(
        &self,
        cursor: Option<McpCursor>,
    ) -> McpResult<McpListPage<McpResource>>;

    async fn read_resource(
        &self,
        request: McpReadResourceRequest,
    ) -> McpResult<McpReadResourceResult>;

    async fn list_prompts(&self, cursor: Option<McpCursor>) -> McpResult<McpListPage<McpPrompt>>;

    async fn get_prompt(&self, request: McpGetPromptRequest) -> McpResult<McpGetPromptResult>;

    async fn create_sampling_message(
        &self,
        request: McpSamplingRequest,
    ) -> McpResult<McpSamplingResult>;

    async fn elicit(&self, request: McpElicitationRequest) -> McpResult<McpElicitationResult>;

    async fn list_roots(&self, request: McpRootsRequest) -> McpResult<Vec<McpRoot>>;
}
