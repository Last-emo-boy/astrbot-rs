use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::McpServerConfig;
use crate::elicitation::{McpElicitationRequest, McpElicitationResult};
use crate::prompts::{McpGetPromptRequest, McpGetPromptResult, McpPrompt};
use crate::resources::{
    McpReadResourceRequest, McpReadResourceResult, McpResource, McpResourceTemplate,
};
use crate::roots::{McpRoot, McpRootResolver, McpRootsRequest};
use crate::sampling::{McpSamplingRequest, McpSamplingResult};
use crate::tools::{McpTool, McpToolCallRequest, McpToolCallResult};
use crate::transport::{McpJsonRpcFrame, McpTransportEndpoint, McpTransportRuntime};
use crate::types::{
    MCP_JSONRPC_VERSION, McpCursor, McpError, McpListPage, McpResult, McpServerName,
};

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
    pub resource_templates: Vec<McpResourceTemplate>,
    pub prompts: Vec<McpPrompt>,
    pub roots: Vec<McpRoot>,
    pub server_capabilities: Option<McpServerCapabilities>,
    pub server_errors: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<Value>,
}

impl McpServerCapabilities {
    pub fn supports_tools(&self) -> bool {
        self.tools.is_some()
    }

    pub fn supports_resources(&self) -> bool {
        self.resources.is_some()
    }

    pub fn supports_prompts(&self) -> bool {
        self.prompts.is_some()
    }
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

    async fn list_resource_templates(
        &self,
        cursor: Option<McpCursor>,
    ) -> McpResult<McpListPage<McpResourceTemplate>>;

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

pub struct McpConcreteClient {
    transport: Arc<dyn McpTransportRuntime>,
    reconnect_policy: McpReconnectPolicy,
    root_base_path: PathBuf,
    state: Mutex<McpConcreteClientState>,
}

impl McpConcreteClient {
    pub fn new(transport: Arc<dyn McpTransportRuntime>) -> Self {
        Self {
            transport,
            reconnect_policy: McpReconnectPolicy::default(),
            root_base_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            state: Mutex::new(McpConcreteClientState::default()),
        }
    }

    pub fn with_reconnect_policy(mut self, reconnect_policy: McpReconnectPolicy) -> Self {
        self.reconnect_policy = reconnect_policy;
        self
    }

    pub fn with_root_base_path(mut self, root_base_path: impl Into<PathBuf>) -> Self {
        self.root_base_path = root_base_path.into();
        self
    }

    async fn request_value<T>(&self, method: &str, params: Value) -> McpResult<T>
    where
        T: DeserializeOwned,
    {
        match self.request_once(method, params.clone()).await {
            Ok(value) => Ok(value),
            Err(err) if is_closed_resource_error(&err) => {
                self.reconnect(self.reconnect_policy.clone()).await?;
                self.request_once(method, params).await
            }
            Err(err) => Err(err),
        }
    }

    async fn request_once<T>(&self, method: &str, params: Value) -> McpResult<T>
    where
        T: DeserializeOwned,
    {
        let (session, read_timeout, id) = {
            let mut state = self.lock_state()?;
            let session = state.session.clone().ok_or_else(|| {
                McpError::NotConnected(format!(
                    "{} is not connected",
                    state
                        .server_name
                        .as_ref()
                        .map(McpServerName::as_str)
                        .unwrap_or("MCP client")
                ))
            })?;
            let read_timeout = state
                .config
                .as_ref()
                .map(McpServerConfig::session_read_timeout)
                .unwrap_or_default();
            let id = state.next_request_id;
            state.next_request_id += 1;
            (session, read_timeout, id)
        };

        let frame = McpJsonRpcFrame {
            value: json!({
                "jsonrpc": MCP_JSONRPC_VERSION,
                "id": id,
                "method": method,
                "params": params,
            }),
        };
        let response = self
            .transport
            .request(&session, frame, read_timeout)
            .await?;
        self.decode_response(response)
    }

    fn decode_response<T>(&self, response: McpJsonRpcFrame) -> McpResult<T>
    where
        T: DeserializeOwned,
    {
        if let Some(error) = response.value.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown MCP protocol error")
                .to_string();
            self.record_server_error(message.clone());
            return Err(McpError::Protocol(message));
        }

        let result = response
            .value
            .get("result")
            .cloned()
            .ok_or_else(|| McpError::Protocol("MCP response missing result".to_string()))?;
        serde_json::from_value(result)
            .map_err(|err| McpError::Protocol(format!("failed to decode MCP response: {err}")))
    }

    async fn refresh_tools(&self) -> McpResult<()> {
        let page = self.list_tools(None).await?;
        self.lock_state()?.tools = page.items;
        Ok(())
    }

    async fn refresh_resources(&self) -> McpResult<()> {
        let page = self.list_resources(None).await?;
        self.lock_state()?.resources = page.items;
        Ok(())
    }

    async fn refresh_resource_templates(&self) -> McpResult<()> {
        let page = self.list_resource_templates(None).await?;
        self.lock_state()?.resource_templates = page.items;
        Ok(())
    }

    async fn refresh_prompts(&self) -> McpResult<()> {
        let page = self.list_prompts(None).await?;
        self.lock_state()?.prompts = page.items;
        Ok(())
    }

    fn lock_state(&self) -> McpResult<MutexGuard<'_, McpConcreteClientState>> {
        self.state
            .lock()
            .map_err(|_| McpError::Protocol("MCP client state lock poisoned".to_string()))
    }

    fn record_server_error(&self, message: String) {
        if let Ok(mut state) = self.state.lock() {
            state.server_errors.push(message);
        }
    }
}

#[async_trait]
impl McpClientLifecycle for McpConcreteClient {
    async fn connect(
        &self,
        server_name: McpServerName,
        config: McpServerConfig,
    ) -> McpResult<McpConnectionReport> {
        let config = config.normalize();
        config.validate()?;
        let endpoint = McpTransportEndpoint::from_server_config(&config)?;
        {
            let mut state = self.lock_state()?;
            state.server_name = Some(server_name.clone());
            state.config = Some(config.clone());
            state.state = McpClientState::Connecting;
            state.server_errors.clear();
        }

        let session = match self.transport.connect(endpoint).await {
            Ok(session) => session,
            Err(err) => {
                self.lock_state()?.state = McpClientState::Failed;
                return Ok(McpConnectionReport::failed(server_name, err.to_string()));
            }
        };

        {
            let mut state = self.lock_state()?;
            state.session = Some(session);
            state.state = McpClientState::Ready;
        }

        let initialize = match self
            .request_value::<McpInitializeResult>(
                "initialize",
                json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": config.client_capabilities,
                    "clientInfo": {
                        "name": "astrbot-rs",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            )
            .await
        {
            Ok(initialize) => initialize,
            Err(err) => {
                self.record_server_error(err.to_string());
                self.lock_state()?.state = McpClientState::Failed;
                return Ok(McpConnectionReport::failed(server_name, err.to_string()));
            }
        };

        {
            let mut state = self.lock_state()?;
            state.server_capabilities = Some(initialize.capabilities.clone());
            state.resources.clear();
            state.resource_templates.clear();
            state.prompts.clear();
        }

        if let Err(err) = self.refresh_tools().await {
            self.record_server_error(err.to_string());
        }
        if initialize.capabilities.supports_resources() {
            if let Err(err) = self.refresh_resources().await {
                self.record_server_error(err.to_string());
            }
            if let Err(err) = self.refresh_resource_templates().await {
                self.record_server_error(err.to_string());
            }
        }
        if initialize.capabilities.supports_prompts()
            && let Err(err) = self.refresh_prompts().await
        {
            self.record_server_error(err.to_string());
        }

        Ok(McpConnectionReport::ready(server_name))
    }

    async fn reconnect(&self, policy: McpReconnectPolicy) -> McpResult<McpConnectionReport> {
        let (server_name, config) = {
            let mut state = self.lock_state()?;
            state.state = McpClientState::Reconnecting;
            (
                state.server_name.clone().ok_or_else(|| {
                    McpError::NotConnected("MCP client has no server name".to_string())
                })?,
                state.config.clone().ok_or_else(|| {
                    McpError::NotConnected("MCP client has no server config".to_string())
                })?,
            )
        };

        let mut last_error = None;
        for attempt in 0..=policy.max_attempts {
            match self.connect(server_name.clone(), config.clone()).await {
                Ok(report) if report.state == McpClientState::Ready => return Ok(report),
                Ok(report) => last_error = report.message,
                Err(err) => last_error = Some(err.to_string()),
            }
            if !crate::transport::mcp_reconnect_decision(&policy, attempt).should_retry() {
                break;
            }
        }

        let message = last_error.unwrap_or_else(|| "MCP reconnect failed".to_string());
        self.lock_state()?.state = McpClientState::Failed;
        Ok(McpConnectionReport::failed(server_name, message))
    }

    async fn shutdown(&self) -> McpResult<()> {
        let session = {
            let mut state = self.lock_state()?;
            state.state = McpClientState::ShuttingDown;
            state.session.take()
        };
        if let Some(session) = session {
            let _ = self
                .transport
                .send(
                    &session,
                    McpJsonRpcFrame {
                        value: json!({
                            "jsonrpc": MCP_JSONRPC_VERSION,
                            "method": "shutdown"
                        }),
                    },
                )
                .await;
            self.transport.close(session).await?;
        }
        self.lock_state()?.state = McpClientState::Disconnected;
        Ok(())
    }

    fn snapshot(&self) -> McpClientSnapshot {
        let Ok(state) = self.state.lock() else {
            return McpClientSnapshot {
                state: McpClientState::Failed,
                ..McpClientSnapshot::default()
            };
        };
        McpClientSnapshot {
            state: state.state,
            tools: state.tools.clone(),
            resources: state.resources.clone(),
            resource_templates: state.resource_templates.clone(),
            prompts: state.prompts.clone(),
            roots: state.roots.clone(),
            server_capabilities: state.server_capabilities.clone(),
            server_errors: state.server_errors.clone(),
        }
    }
}

#[async_trait]
impl McpClientBoundary for McpConcreteClient {
    async fn list_tools(&self, cursor: Option<McpCursor>) -> McpResult<McpListPage<McpTool>> {
        let result: McpToolListResult = self
            .request_value("tools/list", cursor_params(cursor))
            .await?;
        let page = McpListPage {
            items: result.tools,
            next_cursor: result.next_cursor,
        };
        self.lock_state()?.tools = page.items.clone();
        Ok(page)
    }

    async fn call_tool(&self, request: McpToolCallRequest) -> McpResult<McpToolCallResult> {
        self.request_value(
            "tools/call",
            serde_json::to_value(request).map_err(json_error)?,
        )
        .await
    }

    async fn list_resources(
        &self,
        cursor: Option<McpCursor>,
    ) -> McpResult<McpListPage<McpResource>> {
        let result: McpResourceListResult = self
            .request_value("resources/list", cursor_params(cursor))
            .await?;
        let page = McpListPage {
            items: result.resources,
            next_cursor: result.next_cursor,
        };
        self.lock_state()?.resources = page.items.clone();
        Ok(page)
    }

    async fn list_resource_templates(
        &self,
        cursor: Option<McpCursor>,
    ) -> McpResult<McpListPage<McpResourceTemplate>> {
        let result: McpResourceTemplateListResult = self
            .request_value("resources/templates/list", cursor_params(cursor))
            .await?;
        let page = McpListPage {
            items: result.resource_templates,
            next_cursor: result.next_cursor,
        };
        self.lock_state()?.resource_templates = page.items.clone();
        Ok(page)
    }

    async fn read_resource(
        &self,
        request: McpReadResourceRequest,
    ) -> McpResult<McpReadResourceResult> {
        self.request_value(
            "resources/read",
            serde_json::to_value(request).map_err(json_error)?,
        )
        .await
    }

    async fn list_prompts(&self, cursor: Option<McpCursor>) -> McpResult<McpListPage<McpPrompt>> {
        let result: McpPromptListResult = self
            .request_value("prompts/list", cursor_params(cursor))
            .await?;
        let page = McpListPage {
            items: result.prompts,
            next_cursor: result.next_cursor,
        };
        self.lock_state()?.prompts = page.items.clone();
        Ok(page)
    }

    async fn get_prompt(&self, request: McpGetPromptRequest) -> McpResult<McpGetPromptResult> {
        self.request_value(
            "prompts/get",
            serde_json::to_value(request).map_err(json_error)?,
        )
        .await
    }

    async fn create_sampling_message(
        &self,
        request: McpSamplingRequest,
    ) -> McpResult<McpSamplingResult> {
        self.request_value(
            "sampling/createMessage",
            serde_json::to_value(request).map_err(json_error)?,
        )
        .await
    }

    async fn elicit(&self, request: McpElicitationRequest) -> McpResult<McpElicitationResult> {
        self.request_value(
            "elicitation/create",
            serde_json::to_value(request).map_err(json_error)?,
        )
        .await
    }

    async fn list_roots(&self, _request: McpRootsRequest) -> McpResult<Vec<McpRoot>> {
        let config = self
            .lock_state()?
            .config
            .as_ref()
            .map(|config| config.client_capabilities.roots.clone())
            .unwrap_or_default();
        let roots = McpRootResolver::new(self.root_base_path.clone(), config).resolve()?;
        self.lock_state()?.roots = roots.clone();
        Ok(roots)
    }
}

#[derive(Default)]
struct McpConcreteClientState {
    server_name: Option<McpServerName>,
    config: Option<McpServerConfig>,
    session: Option<crate::transport::McpTransportSession>,
    state: McpClientState,
    tools: Vec<McpTool>,
    resources: Vec<McpResource>,
    resource_templates: Vec<McpResourceTemplate>,
    prompts: Vec<McpPrompt>,
    roots: Vec<McpRoot>,
    server_capabilities: Option<McpServerCapabilities>,
    server_errors: Vec<String>,
    next_request_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpInitializeResult {
    #[serde(default)]
    capabilities: McpServerCapabilities,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpToolListResult {
    #[serde(default)]
    tools: Vec<McpTool>,
    next_cursor: Option<McpCursor>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpResourceListResult {
    #[serde(default)]
    resources: Vec<McpResource>,
    next_cursor: Option<McpCursor>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpResourceTemplateListResult {
    #[serde(default)]
    resource_templates: Vec<McpResourceTemplate>,
    next_cursor: Option<McpCursor>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpPromptListResult {
    #[serde(default)]
    prompts: Vec<McpPrompt>,
    next_cursor: Option<McpCursor>,
}

fn cursor_params(cursor: Option<McpCursor>) -> Value {
    cursor
        .map(|cursor| json!({ "cursor": cursor.as_str() }))
        .unwrap_or_else(|| json!({}))
}

fn json_error(err: serde_json::Error) -> McpError {
    McpError::Protocol(format!("failed to encode MCP request: {err}"))
}

fn is_closed_resource_error(err: &McpError) -> bool {
    matches!(err, McpError::Transport(message) if message.to_ascii_lowercase().contains("closed"))
}
