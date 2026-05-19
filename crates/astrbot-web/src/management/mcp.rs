use std::sync::{Arc, RwLock};

use astrbot_mcp::{
    McpConfig, McpServerConfig, McpServerName, build_mcp_prompt_tool_names,
    build_mcp_resource_tool_names,
};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone, Debug, Default)]
pub struct ManagementMcpState {
    inner: Arc<RwLock<McpConfig>>,
}

impl ManagementMcpState {
    pub fn new(config: McpConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(config.normalize())),
        }
    }

    fn catalog_response(&self) -> Result<ManagementMcpCatalogResponse, ManagementMcpError> {
        let config = self
            .inner
            .read()
            .map_err(|error| ManagementMcpError::StateLock(error.to_string()))?;
        Ok(catalog_from_config(&config))
    }

    fn upsert(
        &self,
        request: ManagementMcpUpsertRequest,
    ) -> Result<ManagementMcpMutationResponse, ManagementMcpError> {
        let name = McpServerName::new(request.name).map_err(mcp_error)?;
        let server = request.server.normalize();
        server.validate().map_err(mcp_error)?;

        let mut config = self
            .inner
            .write()
            .map_err(|error| ManagementMcpError::StateLock(error.to_string()))?;
        let changed = config.mcp_servers.0.get(name.as_str()) != Some(&server);
        config
            .mcp_servers
            .0
            .insert(name.as_str().to_string(), server);
        let catalog = catalog_from_config(&config);

        Ok(ManagementMcpMutationResponse { changed, catalog })
    }

    fn delete(
        &self,
        request: ManagementMcpDeleteRequest,
    ) -> Result<ManagementMcpMutationResponse, ManagementMcpError> {
        let name = McpServerName::new(request.name).map_err(mcp_error)?;
        let mut config = self
            .inner
            .write()
            .map_err(|error| ManagementMcpError::StateLock(error.to_string()))?;
        let changed = config.mcp_servers.0.remove(name.as_str()).is_some();
        let catalog = catalog_from_config(&config);

        Ok(ManagementMcpMutationResponse { changed, catalog })
    }

    fn check(
        &self,
        request: ManagementMcpCheckRequest,
    ) -> Result<ManagementMcpCheckResponse, ManagementMcpError> {
        let server = if let Some(server) = request.server {
            server.normalize()
        } else {
            let name = request.name.ok_or_else(|| {
                ManagementMcpError::Invalid("check requires name or server".to_string())
            })?;
            let name = McpServerName::new(name).map_err(mcp_error)?;
            let config = self
                .inner
                .read()
                .map_err(|error| ManagementMcpError::StateLock(error.to_string()))?;
            config
                .mcp_servers
                .0
                .get(name.as_str())
                .cloned()
                .ok_or_else(|| ManagementMcpError::NotFound(name.as_str().to_string()))?
                .normalize()
        };
        match server.validate() {
            Ok(()) => Ok(ManagementMcpCheckResponse {
                ok: true,
                message: "MCP server config is valid; process/network connectivity is not probed."
                    .to_string(),
                server: ManagementMcpServerConfigView::from_config(&server),
            }),
            Err(error) => Ok(ManagementMcpCheckResponse {
                ok: false,
                message: error.to_string(),
                server: ManagementMcpServerConfigView::from_config(&server),
            }),
        }
    }

    fn sync_plan(
        &self,
        request: ManagementMcpSyncRequest,
    ) -> Result<ManagementMcpSyncResponse, ManagementMcpError> {
        let config = self
            .inner
            .read()
            .map_err(|error| ManagementMcpError::StateLock(error.to_string()))?;
        let requested_names = request.names.unwrap_or_default();
        let requested_names = requested_names
            .into_iter()
            .map(|name| McpServerName::new(name).map_err(mcp_error))
            .collect::<Result<Vec<_>, _>>()?;
        let mut synced_servers = Vec::new();
        let mut bridge_tools = Vec::new();

        for (name, server) in &config.mcp_servers.0 {
            if !server.active {
                continue;
            }
            if !requested_names.is_empty()
                && !requested_names
                    .iter()
                    .any(|requested| requested.as_str() == name)
            {
                continue;
            }
            server.validate().map_err(mcp_error)?;
            synced_servers.push(name.clone());
            bridge_tools.extend(build_mcp_resource_tool_names(name, true));
            bridge_tools.extend(build_mcp_prompt_tool_names(name));
        }
        bridge_tools.sort();
        bridge_tools.dedup();

        Ok(ManagementMcpSyncResponse {
            synced_servers,
            bridge_tools,
            message: "MCP sync generated a configuration-only bridge plan; live tool discovery is not probed."
                .to_string(),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementMcpCatalogResponse {
    pub servers: Vec<ManagementMcpServerDescriptor>,
    pub active_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementMcpServerDescriptor {
    pub name: String,
    pub active: bool,
    pub transport: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
    pub headers_configured: bool,
    pub session_read_timeout_seconds: u64,
    pub client_capabilities: Value,
    pub valid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementMcpUpsertRequest {
    pub name: String,
    pub server: McpServerConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementMcpDeleteRequest {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementMcpCheckRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub server: Option<McpServerConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementMcpSyncRequest {
    #[serde(default)]
    pub names: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementMcpMutationResponse {
    pub changed: bool,
    pub catalog: ManagementMcpCatalogResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementMcpCheckResponse {
    pub ok: bool,
    pub message: String,
    pub server: ManagementMcpServerConfigView,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementMcpServerConfigView {
    pub active: bool,
    pub transport: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
    pub headers_configured: bool,
    pub session_read_timeout_seconds: u64,
    pub client_capabilities: Value,
}

impl ManagementMcpServerConfigView {
    fn from_config(config: &McpServerConfig) -> Self {
        Self {
            active: config.active,
            transport: serde_json::to_value(&config.transport)
                .ok()
                .and_then(|value| value.as_str().map(ToString::to_string))
                .unwrap_or_else(|| "stdio".to_string()),
            command: config.command.clone(),
            args: config.args.clone(),
            url: config.url.clone(),
            headers_configured: !config.headers.is_empty(),
            session_read_timeout_seconds: config.session_read_timeout_seconds,
            client_capabilities: serde_json::to_value(&config.client_capabilities)
                .unwrap_or(Value::Null),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementMcpSyncResponse {
    pub synced_servers: Vec<String>,
    pub bridge_tools: Vec<String>,
    pub message: String,
}

pub async fn catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementMcpCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mcp = state.mcp().ok_or_else(mcp_state_unavailable)?;
    mcp.catalog_response().map(Json).map_err(map_mcp_error)
}

pub async fn upsert(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementMcpUpsertRequest>,
) -> Result<Json<ManagementMcpMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mcp = state.mcp().ok_or_else(mcp_state_unavailable)?;
    mcp.upsert(request).map(Json).map_err(map_mcp_error)
}

pub async fn delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementMcpDeleteRequest>,
) -> Result<Json<ManagementMcpMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mcp = state.mcp().ok_or_else(mcp_state_unavailable)?;
    mcp.delete(request).map(Json).map_err(map_mcp_error)
}

pub async fn check(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementMcpCheckRequest>,
) -> Result<Json<ManagementMcpCheckResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mcp = state.mcp().ok_or_else(mcp_state_unavailable)?;
    mcp.check(request).map(Json).map_err(map_mcp_error)
}

pub async fn sync(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementMcpSyncRequest>,
) -> Result<Json<ManagementMcpSyncResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mcp = state.mcp().ok_or_else(mcp_state_unavailable)?;
    mcp.sync_plan(request).map(Json).map_err(map_mcp_error)
}

pub async fn legacy_catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let mcp = state.mcp().ok_or_else(mcp_state_unavailable)?;
    let catalog = mcp.catalog_response().map_err(map_mcp_error)?;
    Ok(source_ok(json!(catalog.servers)))
}

pub async fn legacy_add(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let mcp = state.mcp().ok_or_else(mcp_state_unavailable)?;
    let (_old_name, request) = legacy_upsert_request(payload).map_err(map_mcp_error)?;
    let response = mcp.upsert(request).map_err(map_mcp_error)?;
    Ok(source_ok(json!(response.catalog.servers)))
}

pub async fn legacy_update(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let mcp = state.mcp().ok_or_else(mcp_state_unavailable)?;
    let (old_name, request) = legacy_upsert_request(payload).map_err(map_mcp_error)?;
    let new_name = request.name.clone();
    let response = mcp.upsert(request).map_err(map_mcp_error)?;
    if let Some(old_name) = old_name
        && old_name != new_name
    {
        let _ = mcp.delete(ManagementMcpDeleteRequest { name: old_name });
    }
    Ok(source_ok(json!(response.catalog.servers)))
}

pub async fn legacy_delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementMcpDeleteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let mcp = state.mcp().ok_or_else(mcp_state_unavailable)?;
    let response = mcp.delete(request).map_err(map_mcp_error)?;
    Ok(source_ok(json!(response.catalog.servers)))
}

pub async fn legacy_check(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let mcp = state.mcp().ok_or_else(mcp_state_unavailable)?;
    let request = legacy_check_request(payload).map_err(map_mcp_error)?;
    let response = mcp.check(request).map_err(map_mcp_error)?;
    Ok(source_ok(json!(response)))
}

pub async fn legacy_sync_provider(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let mcp = state.mcp().ok_or_else(mcp_state_unavailable)?;
    let names = payload
        .get("names")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .or_else(|| {
            payload
                .get("name")
                .or_else(|| payload.get("server_name"))
                .and_then(Value::as_str)
                .map(|name| vec![name.to_string()])
        });
    let response = mcp
        .sync_plan(ManagementMcpSyncRequest { names })
        .map_err(map_mcp_error)?;
    Ok(source_ok(json!(response)))
}

fn catalog_from_config(config: &McpConfig) -> ManagementMcpCatalogResponse {
    let servers = config
        .mcp_servers
        .0
        .iter()
        .map(|(name, server)| server_descriptor(name, server))
        .collect::<Vec<_>>();
    let active_count = servers.iter().filter(|server| server.active).count();

    ManagementMcpCatalogResponse {
        servers,
        active_count,
    }
}

fn server_descriptor(name: &str, server: &McpServerConfig) -> ManagementMcpServerDescriptor {
    let validation_error = server.validate().err().map(|error| error.to_string());
    let view = ManagementMcpServerConfigView::from_config(server);
    ManagementMcpServerDescriptor {
        name: name.to_string(),
        active: view.active,
        transport: view.transport,
        command: view.command,
        args: view.args,
        url: view.url,
        headers_configured: view.headers_configured,
        session_read_timeout_seconds: view.session_read_timeout_seconds,
        client_capabilities: view.client_capabilities,
        valid: validation_error.is_none(),
        validation_error,
    }
}

fn legacy_upsert_request(
    mut payload: Value,
) -> Result<(Option<String>, ManagementMcpUpsertRequest), ManagementMcpError> {
    let object = payload
        .as_object_mut()
        .ok_or_else(|| ManagementMcpError::Invalid("MCP payload must be an object".to_string()))?;
    let name = object
        .remove("name")
        .and_then(|value| value.as_str().map(ToString::to_string))
        .ok_or_else(|| ManagementMcpError::Invalid("MCP server name is required".to_string()))?;
    let old_name = object
        .remove("oldName")
        .or_else(|| object.remove("old_name"))
        .and_then(|value| value.as_str().map(ToString::to_string));
    object.remove("tools");
    object.remove("errlogs");

    let server_value = if let Some(mcp_servers) = object.remove("mcpServers") {
        mcp_servers
            .as_object()
            .and_then(|servers| servers.values().next().cloned())
            .ok_or_else(|| {
                ManagementMcpError::Invalid("mcpServers must contain one server".to_string())
            })?
    } else {
        Value::Object(object.clone())
    };
    let server = serde_json::from_value::<McpServerConfig>(server_value)
        .map_err(|error| ManagementMcpError::Invalid(format!("parse MCP server config: {error}")))?
        .normalize();

    Ok((old_name, ManagementMcpUpsertRequest { name, server }))
}

fn legacy_check_request(payload: Value) -> Result<ManagementMcpCheckRequest, ManagementMcpError> {
    if let Some(name) = payload.get("name").and_then(Value::as_str) {
        return Ok(ManagementMcpCheckRequest {
            name: Some(name.to_string()),
            server: None,
        });
    }
    if let Some(server) = payload.get("server").cloned() {
        let server = serde_json::from_value::<McpServerConfig>(server).map_err(|error| {
            ManagementMcpError::Invalid(format!("parse MCP server config: {error}"))
        })?;
        return Ok(ManagementMcpCheckRequest {
            name: None,
            server: Some(server),
        });
    }
    let mut object = payload.as_object().cloned().unwrap_or_else(Map::new);
    object.remove("tools");
    object.remove("errlogs");
    object.remove("name");
    let server =
        serde_json::from_value::<McpServerConfig>(Value::Object(object)).map_err(|error| {
            ManagementMcpError::Invalid(format!("parse MCP server config: {error}"))
        })?;
    Ok(ManagementMcpCheckRequest {
        name: None,
        server: Some(server),
    })
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": "ok",
        "data": data,
    }))
}

#[derive(Debug)]
enum ManagementMcpError {
    StateLock(String),
    Invalid(String),
    NotFound(String),
}

fn mcp_error(error: astrbot_mcp::McpError) -> ManagementMcpError {
    ManagementMcpError::Invalid(error.to_string())
}

fn mcp_state_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "MCP management state is not configured".to_string(),
        }),
    )
}

fn map_mcp_error(error: ManagementMcpError) -> (StatusCode, Json<ErrorResponse>) {
    let (status, message) = match error {
        ManagementMcpError::StateLock(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("MCP management state lock: {message}"),
        ),
        ManagementMcpError::Invalid(message) => (StatusCode::BAD_REQUEST, message),
        ManagementMcpError::NotFound(name) => (
            StatusCode::NOT_FOUND,
            format!("MCP server {name} not found"),
        ),
    };

    (status, Json(ErrorResponse { error: message }))
}
