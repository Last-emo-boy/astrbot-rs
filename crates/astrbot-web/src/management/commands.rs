use astrbot_runtime::{RuntimeCommandPluginConfig, RuntimeConfigService};
use astrbot_tool::{
    CommandConflict, CommandDescriptor, CommandPermission, CommandType, detect_command_conflicts,
};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementCommandCatalogResponse {
    pub commands: Vec<ManagementCommandDescriptor>,
    pub conflicts: Vec<CommandConflict>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementCommandDescriptor {
    pub handler_full_name: String,
    pub plugin_name: String,
    pub handler_name: String,
    pub description: String,
    pub command_type: CommandType,
    pub original_command: String,
    pub current_fragment: String,
    pub effective_command: String,
    pub aliases: Vec<String>,
    pub effective_aliases: Vec<String>,
    pub permission: CommandPermission,
    pub enabled: bool,
    pub reserved: bool,
    pub response: String,
    pub priority: i32,
}

impl ManagementCommandDescriptor {
    fn from_runtime_config(config: &RuntimeCommandPluginConfig) -> Self {
        let descriptor = command_descriptor_from_runtime_config(config);
        let effective_command = descriptor.effective_command();
        let effective_aliases = descriptor.effective_aliases();
        Self {
            handler_full_name: descriptor.handler_full_name.clone(),
            plugin_name: descriptor.plugin_name.clone(),
            handler_name: config.handler_name.clone(),
            description: descriptor.description.clone(),
            command_type: descriptor.command_type,
            original_command: descriptor.original_command.clone(),
            current_fragment: descriptor.current_fragment.clone(),
            effective_command,
            aliases: descriptor.aliases.clone(),
            effective_aliases,
            permission: descriptor.permission,
            enabled: descriptor.enabled,
            reserved: descriptor.reserved,
            response: config.response.clone(),
            priority: config.priority,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementCommandUpdateRequest {
    pub plugin_name: String,
    pub handler_name: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub response: Option<String>,
    #[serde(default)]
    pub priority: Option<i32>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub permission: Option<CommandPermission>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementCommandMutationResponse {
    pub changed: bool,
    pub command: ManagementCommandDescriptor,
    pub catalog: ManagementCommandCatalogResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyCommandToggleRequest {
    pub handler_full_name: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyCommandRenameRequest {
    pub handler_full_name: String,
    pub new_name: String,
    #[serde(default)]
    pub aliases: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyCommandPermissionRequest {
    pub handler_full_name: String,
    pub permission: CommandPermission,
}

pub async fn catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementCommandCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let config_service = state
        .config_service()
        .ok_or_else(command_state_unavailable)?;
    command_catalog_response(config_service)
        .map(Json)
        .map_err(map_command_error)
}

pub async fn update(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementCommandUpdateRequest>,
) -> Result<Json<ManagementCommandMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let config_service = state
        .config_service()
        .ok_or_else(command_state_unavailable)?;
    update_command(config_service, request)
        .map(Json)
        .map_err(map_command_error)
}

pub async fn legacy_catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let config_service = state
        .config_service()
        .ok_or_else(command_state_unavailable)?;
    let catalog = command_catalog_response(config_service).map_err(map_command_error)?;
    let items = catalog
        .commands
        .iter()
        .map(legacy_command_value)
        .collect::<Vec<_>>();
    Ok(source_ok(json!({
        "items": items,
        "summary": {
            "total": catalog.commands.len(),
            "disabled": catalog.commands.iter().filter(|command| !command.enabled).count(),
            "conflicts": catalog.conflicts.len(),
        },
    })))
}

pub async fn legacy_conflicts(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let config_service = state
        .config_service()
        .ok_or_else(command_state_unavailable)?;
    let catalog = command_catalog_response(config_service).map_err(map_command_error)?;
    Ok(source_ok(json!(catalog.conflicts)))
}

pub async fn legacy_toggle(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyCommandToggleRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let config_service = state
        .config_service()
        .ok_or_else(command_state_unavailable)?;
    let (plugin_name, handler_name) = split_handler_full_name(&request.handler_full_name)?;
    let updated = update_command(
        config_service,
        ManagementCommandUpdateRequest {
            plugin_name,
            handler_name,
            command: None,
            response: None,
            priority: None,
            enabled: Some(request.enabled),
            permission: None,
        },
    )
    .map_err(map_command_error)?;
    Ok(source_ok(legacy_command_value(&updated.command)))
}

pub async fn legacy_rename(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyCommandRenameRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let config_service = state
        .config_service()
        .ok_or_else(command_state_unavailable)?;
    let (plugin_name, handler_name) = split_handler_full_name(&request.handler_full_name)?;
    let updated = update_command(
        config_service,
        ManagementCommandUpdateRequest {
            plugin_name,
            handler_name,
            command: Some(request.new_name),
            response: None,
            priority: None,
            enabled: None,
            permission: None,
        },
    )
    .map_err(map_command_error)?;
    Ok(source_ok(legacy_command_value(&updated.command)))
}

pub async fn legacy_permission(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyCommandPermissionRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let config_service = state
        .config_service()
        .ok_or_else(command_state_unavailable)?;
    let (plugin_name, handler_name) = split_handler_full_name(&request.handler_full_name)?;
    let updated = update_command(
        config_service,
        ManagementCommandUpdateRequest {
            plugin_name,
            handler_name,
            command: None,
            response: None,
            priority: None,
            enabled: None,
            permission: Some(request.permission),
        },
    )
    .map_err(map_command_error)?;
    Ok(source_ok(legacy_command_value(&updated.command)))
}

fn command_catalog_response(
    config_service: &RuntimeConfigService,
) -> Result<ManagementCommandCatalogResponse, ManagementCommandError> {
    let config = config_service
        .read_config()
        .map_err(|error| ManagementCommandError::Config(error.to_string()))?;
    let descriptors = config
        .command_plugins
        .iter()
        .map(command_descriptor_from_runtime_config)
        .collect::<Vec<_>>();
    let commands = config
        .command_plugins
        .iter()
        .map(ManagementCommandDescriptor::from_runtime_config)
        .collect::<Vec<_>>();
    let conflicts = detect_command_conflicts(&descriptors);

    Ok(ManagementCommandCatalogResponse {
        commands,
        conflicts,
    })
}

fn update_command(
    config_service: &RuntimeConfigService,
    request: ManagementCommandUpdateRequest,
) -> Result<ManagementCommandMutationResponse, ManagementCommandError> {
    let plugin_name = normalized_required(request.plugin_name, "plugin_name")?;
    let handler_name = normalized_required(request.handler_name, "handler_name")?;
    let command = request.command.map(|command| command.trim().to_string());
    if command.as_ref().is_some_and(|command| command.is_empty()) {
        return Err(ManagementCommandError::Invalid(
            "command cannot be empty".to_string(),
        ));
    }

    let mut config = config_service
        .read_config()
        .map_err(|error| ManagementCommandError::Config(error.to_string()))?;
    let index = config
        .command_plugins
        .iter()
        .position(|item| item.plugin_name == plugin_name && item.handler_name == handler_name);
    let mut changed = false;

    let command_config = if let Some(index) = index {
        &mut config.command_plugins[index]
    } else {
        let initial_command = command.clone().ok_or_else(|| {
            ManagementCommandError::Invalid("new command requires command".to_string())
        })?;
        config.command_plugins.push(RuntimeCommandPluginConfig {
            plugin_name: plugin_name.clone(),
            handler_name: handler_name.clone(),
            command: initial_command,
            response: request.response.clone().unwrap_or_default(),
            priority: request.priority.unwrap_or_default(),
            enabled: request.enabled.unwrap_or(true),
            permission: request.permission.unwrap_or_default(),
        });
        changed = true;
        config
            .command_plugins
            .last_mut()
            .expect("pushed command config should exist")
    };

    if let Some(command) = command {
        changed |= command_config.command != command;
        command_config.command = command;
    }
    if let Some(response) = request.response {
        changed |= command_config.response != response;
        command_config.response = response;
    }
    if let Some(priority) = request.priority {
        changed |= command_config.priority != priority;
        command_config.priority = priority;
    }
    if let Some(enabled) = request.enabled {
        changed |= command_config.enabled != enabled;
        command_config.enabled = enabled;
    }
    if let Some(permission) = request.permission {
        changed |= command_config.permission != permission;
        command_config.permission = permission;
    }

    let command = ManagementCommandDescriptor::from_runtime_config(command_config);
    let candidate = serde_json::to_value(config)
        .map_err(|error| ManagementCommandError::Config(error.to_string()))?;
    config_service
        .save_update_value(candidate)
        .map_err(|error| ManagementCommandError::Config(error.to_string()))?;
    let catalog = command_catalog_response(config_service)?;

    Ok(ManagementCommandMutationResponse {
        changed,
        command,
        catalog,
    })
}

fn command_descriptor_from_runtime_config(
    config: &RuntimeCommandPluginConfig,
) -> CommandDescriptor {
    CommandDescriptor::new(
        format!("{}.{}", config.plugin_name, config.handler_name),
        config.plugin_name.clone(),
        config.command.clone(),
    )
    .with_permission(config.permission)
}

fn normalized_required(value: String, field: &str) -> Result<String, ManagementCommandError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(ManagementCommandError::Invalid(format!(
            "{field} cannot be empty"
        )));
    }
    Ok(value)
}

fn split_handler_full_name(
    handler_full_name: &str,
) -> Result<(String, String), (StatusCode, Json<ErrorResponse>)> {
    let Some((plugin_name, handler_name)) = handler_full_name.split_once('.') else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "handler_full_name must be plugin.handler".to_string(),
            }),
        ));
    };
    Ok((plugin_name.to_string(), handler_name.to_string()))
}

fn legacy_command_value(command: &ManagementCommandDescriptor) -> Value {
    json!({
        "handler_full_name": command.handler_full_name,
        "plugin_name": command.plugin_name,
        "handler_name": command.handler_name,
        "description": command.description,
        "command_type": command.command_type,
        "original_command": command.original_command,
        "current_command": command.effective_command,
        "current_fragment": command.current_fragment,
        "effective_command": command.effective_command,
        "aliases": command.aliases,
        "effective_aliases": command.effective_aliases,
        "permission": command.permission,
        "enabled": command.enabled,
        "reserved": command.reserved,
        "response": command.response,
        "priority": command.priority,
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
enum ManagementCommandError {
    Config(String),
    Invalid(String),
}

fn command_state_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "command management requires runtime config service".to_string(),
        }),
    )
}

fn map_command_error(error: ManagementCommandError) -> (StatusCode, Json<ErrorResponse>) {
    let (status, message) = match error {
        ManagementCommandError::Config(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("command management config: {message}"),
        ),
        ManagementCommandError::Invalid(message) => (StatusCode::BAD_REQUEST, message),
    };

    (status, Json(ErrorResponse { error: message }))
}
