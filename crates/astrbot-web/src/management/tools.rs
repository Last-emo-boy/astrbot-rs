use std::sync::{Arc, RwLock};

use astrbot_tool::{ToolActivationPolicy, ToolCatalog, ToolDescriptor};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone, Debug)]
pub struct ManagementToolState {
    inner: Arc<RwLock<ManagementToolSnapshot>>,
}

impl ManagementToolState {
    pub fn new(catalog: ToolCatalog) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ManagementToolSnapshot {
                catalog,
                activation: ToolActivationPolicy::new(),
            })),
        }
    }

    pub fn with_activation_policy(self, activation: ToolActivationPolicy) -> Self {
        if let Ok(mut snapshot) = self.inner.write() {
            snapshot.activation = activation;
        }
        self
    }

    fn catalog_response(&self) -> Result<ManagementToolCatalogResponse, ManagementToolError> {
        let snapshot = self.read_snapshot()?;
        let tools = snapshot
            .catalog
            .tools()
            .iter()
            .map(|tool| ManagementToolDescriptor::from_descriptor(tool, &snapshot.activation))
            .collect();

        Ok(ManagementToolCatalogResponse { tools })
    }

    fn set_active(
        &self,
        name: String,
        active: bool,
    ) -> Result<ManagementToolToggleResponse, ManagementToolError> {
        let mut snapshot = self.write_snapshot()?;
        let tool = snapshot
            .catalog
            .tool(&name)
            .ok_or_else(|| ManagementToolError::ToolNotFound { name: name.clone() })?
            .clone();

        if !tool.source.allows_user_toggle() {
            return Err(ManagementToolError::ToggleDenied { name });
        }

        snapshot.activation = snapshot
            .activation
            .clone()
            .set_enabled(name.clone(), active);
        let active = tool.active && snapshot.activation.is_enabled_for(&tool);
        Ok(ManagementToolToggleResponse {
            name,
            active,
            user_toggle_allowed: tool.source.allows_user_toggle(),
        })
    }

    fn read_snapshot(
        &self,
    ) -> Result<std::sync::RwLockReadGuard<'_, ManagementToolSnapshot>, ManagementToolError> {
        self.inner
            .read()
            .map_err(|error| ManagementToolError::StateLock(error.to_string()))
    }

    fn write_snapshot(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, ManagementToolSnapshot>, ManagementToolError> {
        self.inner
            .write()
            .map_err(|error| ManagementToolError::StateLock(error.to_string()))
    }
}

#[derive(Clone, Debug)]
struct ManagementToolSnapshot {
    catalog: ToolCatalog,
    activation: ToolActivationPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementToolCatalogResponse {
    pub tools: Vec<ManagementToolDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementToolDescriptor {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: Value,
    pub active: bool,
    pub origin: String,
    pub origin_name: String,
    pub source: String,
    pub user_toggle_allowed: bool,
}

impl ManagementToolDescriptor {
    fn from_descriptor(descriptor: &ToolDescriptor, activation: &ToolActivationPolicy) -> Self {
        Self {
            name: descriptor.name.clone(),
            description: descriptor.description.clone(),
            parameters: descriptor.parameters.clone(),
            active: descriptor.active && activation.is_enabled_for(descriptor),
            origin: descriptor.source.origin().to_string(),
            origin_name: descriptor.source.origin_name().to_string(),
            source: descriptor.source.source_label().to_string(),
            user_toggle_allowed: descriptor.source.allows_user_toggle(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementToolToggleRequest {
    pub name: String,
    #[serde(alias = "activate")]
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementToolToggleResponse {
    pub name: String,
    pub active: bool,
    pub user_toggle_allowed: bool,
}

#[derive(Debug)]
enum ManagementToolError {
    StateLock(String),
    ToolNotFound { name: String },
    ToggleDenied { name: String },
}

pub async fn catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementToolCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let tools = state.tools().ok_or_else(tool_state_unavailable)?;
    tools.catalog_response().map(Json).map_err(map_tool_error)
}

pub async fn toggle(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementToolToggleRequest>,
) -> Result<Json<ManagementToolToggleResponse>, (StatusCode, Json<ErrorResponse>)> {
    let tools = state.tools().ok_or_else(tool_state_unavailable)?;
    tools
        .set_active(request.name, request.active)
        .map(Json)
        .map_err(map_tool_error)
}

fn tool_state_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "tool management state is not configured".to_string(),
        }),
    )
}

fn map_tool_error(error: ManagementToolError) -> (StatusCode, Json<ErrorResponse>) {
    let (status, message) = match error {
        ManagementToolError::StateLock(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("tool management state lock: {message}"),
        ),
        ManagementToolError::ToolNotFound { name } => {
            (StatusCode::NOT_FOUND, format!("tool {name} not found"))
        }
        ManagementToolError::ToggleDenied { name } => (
            StatusCode::FORBIDDEN,
            format!("internal tool {name} does not allow manual toggle"),
        ),
    };

    (
        status,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
}
