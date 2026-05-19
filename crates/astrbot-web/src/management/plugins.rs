use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_plugin::{
    InMemoryPluginStore, PluginLifecycleAction, PluginLifecycleEvent, PluginLifecycleState,
    PluginLoadSource, PluginLoadSourceKind, PluginLoader, PluginManifest, PluginRecord,
    PluginRegistry, PluginStateStore,
};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManagementResponse {
    pub handler_count: usize,
    pub handlers: Vec<PluginHandlerManagementResponse>,
}

impl PluginManagementResponse {
    pub fn from_registry(registry: &PluginRegistry) -> Self {
        Self {
            handler_count: registry.handler_count(),
            handlers: registry
                .handlers()
                .iter()
                .map(|handler| {
                    let metadata = handler.metadata();
                    PluginHandlerManagementResponse {
                        plugin_name: metadata.plugin_name.clone(),
                        handler_name: metadata.handler_name.clone(),
                        event_type: format!("{:?}", metadata.event_type),
                        priority: metadata.priority,
                        enabled: metadata.enabled,
                    }
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginHandlerManagementResponse {
    pub plugin_name: String,
    pub handler_name: String,
    pub event_type: String,
    pub priority: i32,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct ManagementPluginLifecycleState {
    inner: Arc<RwLock<ManagementPluginLifecycleSnapshot>>,
}

impl std::fmt::Debug for ManagementPluginLifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagementPluginLifecycleState")
            .finish_non_exhaustive()
    }
}

impl ManagementPluginLifecycleState {
    pub fn new(records: Vec<ManagementPluginSeed>) -> Self {
        let mut loader = PluginLoader::new();
        for seed in records {
            let Ok(metadata) =
                astrbot_plugin::PluginMetadata::from_manifest(seed.source, seed.manifest)
            else {
                continue;
            };
            loader
                .store_mut()
                .upsert(PluginRecord::new(metadata, seed.state));
        }
        Self {
            inner: Arc::new(RwLock::new(ManagementPluginLifecycleSnapshot {
                loader,
                configs: BTreeMap::new(),
                operations: Vec::new(),
                next_operation_seq: 1,
            })),
        }
    }

    fn catalog_response(
        &self,
        handlers: PluginManagementResponse,
    ) -> Result<ManagementPluginLifecycleCatalogResponse, ManagementPluginLifecycleError> {
        let snapshot = self
            .inner
            .read()
            .map_err(|error| ManagementPluginLifecycleError::StateLock(error.to_string()))?;
        Ok(snapshot.catalog_response(handlers))
    }

    fn apply_action(
        &self,
        request: ManagementPluginLifecycleActionRequest,
        handlers: PluginManagementResponse,
    ) -> Result<ManagementPluginLifecycleMutationResponse, ManagementPluginLifecycleError> {
        let mut snapshot = self
            .inner
            .write()
            .map_err(|error| ManagementPluginLifecycleError::StateLock(error.to_string()))?;
        let event = match request.action {
            ManagementPluginAction::Load => snapshot.loader.mark_loaded(&request.plugin_id),
            ManagementPluginAction::Activate => snapshot.loader.activate(&request.plugin_id),
            ManagementPluginAction::Disable => snapshot.loader.disable(&request.plugin_id),
            ManagementPluginAction::Reload => snapshot.loader.reload(&request.plugin_id),
            ManagementPluginAction::Unload => snapshot.loader.unload(&request.plugin_id),
            ManagementPluginAction::Fail => snapshot.loader.mark_failed(&request.plugin_id),
        }
        .map_err(|error| ManagementPluginLifecycleError::Invalid(error.to_string()))?;
        let event = ManagementPluginLifecycleEventDescriptor::from_event(event);
        let operation = snapshot.record_operation(
            request.plugin_id,
            format!("{:?}", request.action).to_ascii_lowercase(),
            format!(
                "plugin transitioned from {} to {}",
                event.previous, event.next
            ),
        );
        let catalog = snapshot.catalog_response(handlers);

        Ok(ManagementPluginLifecycleMutationResponse {
            event: Some(event),
            operation,
            catalog,
        })
    }

    fn upload_plan(
        &self,
        request: ManagementPluginUploadPlanRequest,
    ) -> Result<ManagementPluginUploadPlanResponse, ManagementPluginLifecycleError> {
        plugin_upload_plan(request)
    }

    fn source_plan(
        &self,
        request: ManagementPluginSourcePlanRequest,
    ) -> Result<ManagementPluginSourcePlanResponse, ManagementPluginLifecycleError> {
        let source = source_from_request(&request)?;
        Ok(ManagementPluginSourcePlanResponse {
            source: ManagementPluginSourceDescriptor::from_source(&source),
            message: "Plugin source plan is configuration-only; no download/import is executed."
                .to_string(),
        })
    }

    fn save_config(
        &self,
        request: ManagementPluginConfigRequest,
        handlers: PluginManagementResponse,
    ) -> Result<ManagementPluginLifecycleMutationResponse, ManagementPluginLifecycleError> {
        let mut snapshot = self
            .inner
            .write()
            .map_err(|error| ManagementPluginLifecycleError::StateLock(error.to_string()))?;
        if snapshot.loader.store().get(&request.plugin_id).is_none() {
            return Err(ManagementPluginLifecycleError::NotFound(request.plugin_id));
        }
        snapshot
            .configs
            .insert(request.plugin_id.clone(), request.config);
        let operation = snapshot.record_operation(
            request.plugin_id,
            "config".to_string(),
            "plugin config saved in management state".to_string(),
        );
        let catalog = snapshot.catalog_response(handlers);

        Ok(ManagementPluginLifecycleMutationResponse {
            event: None,
            operation,
            catalog,
        })
    }

    fn list_config_files(
        &self,
        request: ManagementPluginConfigFileListRequest,
    ) -> Result<ManagementPluginConfigFileListResponse, ManagementPluginLifecycleError> {
        let root = self.plugin_root(&request.plugin_id)?;
        Ok(ManagementPluginConfigFileListResponse {
            plugin_id: request.plugin_id,
            files: config_files_in_root(&root),
        })
    }

    fn read_config_file(
        &self,
        request: ManagementPluginConfigFileRequest,
    ) -> Result<ManagementPluginConfigFileReadResponse, ManagementPluginLifecycleError> {
        let filename = validate_config_filename(&request.filename)?;
        let path = self.plugin_config_file_path(&request.plugin_id, &filename)?;
        if !path.is_file() {
            return Err(ManagementPluginLifecycleError::NotFound(format!(
                "{}:{filename}",
                request.plugin_id
            )));
        }
        let bytes = fs::read(&path)
            .map_err(|error| ManagementPluginLifecycleError::Io(error.to_string()))?;
        let config = serde_json::from_slice(&bytes)
            .map_err(|error| ManagementPluginLifecycleError::Invalid(error.to_string()))?;
        Ok(ManagementPluginConfigFileReadResponse {
            plugin_id: request.plugin_id,
            filename,
            config,
        })
    }

    fn write_config_file(
        &self,
        request: ManagementPluginConfigFileWriteRequest,
    ) -> Result<ManagementPluginConfigFileWriteResponse, ManagementPluginLifecycleError> {
        let filename = validate_config_filename(&request.filename)?;
        let path = self.plugin_config_file_path(&request.plugin_id, &filename)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| ManagementPluginLifecycleError::Io(error.to_string()))?;
        }
        let bytes = serde_json::to_vec_pretty(&request.config)
            .map_err(|error| ManagementPluginLifecycleError::Invalid(error.to_string()))?;
        fs::write(&path, bytes)
            .map_err(|error| ManagementPluginLifecycleError::Io(error.to_string()))?;

        if filename == "config.json" {
            let mut snapshot = self
                .inner
                .write()
                .map_err(|error| ManagementPluginLifecycleError::StateLock(error.to_string()))?;
            snapshot
                .configs
                .insert(request.plugin_id.clone(), request.config.clone());
        }

        Ok(ManagementPluginConfigFileWriteResponse {
            plugin_id: request.plugin_id,
            file: config_file_descriptor(&path, filename)?,
            config: request.config,
        })
    }

    fn delete_config_file(
        &self,
        request: ManagementPluginConfigFileRequest,
    ) -> Result<ManagementPluginConfigFileDeleteResponse, ManagementPluginLifecycleError> {
        let filename = validate_config_filename(&request.filename)?;
        let path = self.plugin_config_file_path(&request.plugin_id, &filename)?;
        if !path.exists() {
            return Ok(ManagementPluginConfigFileDeleteResponse {
                plugin_id: request.plugin_id,
                filename,
                deleted: false,
            });
        }
        if !path.is_file() {
            return Err(ManagementPluginLifecycleError::Invalid(
                "plugin config path is not a file".to_string(),
            ));
        }
        fs::remove_file(path)
            .map_err(|error| ManagementPluginLifecycleError::Io(error.to_string()))?;
        if filename == "config.json" {
            let mut snapshot = self
                .inner
                .write()
                .map_err(|error| ManagementPluginLifecycleError::StateLock(error.to_string()))?;
            snapshot.configs.remove(&request.plugin_id);
        }
        Ok(ManagementPluginConfigFileDeleteResponse {
            plugin_id: request.plugin_id,
            filename,
            deleted: true,
        })
    }

    fn plugin_root(&self, plugin_id: &str) -> Result<PathBuf, ManagementPluginLifecycleError> {
        let snapshot = self
            .inner
            .read()
            .map_err(|error| ManagementPluginLifecycleError::StateLock(error.to_string()))?;
        let record = snapshot
            .loader
            .store()
            .get(plugin_id)
            .ok_or_else(|| ManagementPluginLifecycleError::NotFound(plugin_id.to_string()))?;
        record.metadata.source.root_dir().cloned().ok_or_else(|| {
            ManagementPluginLifecycleError::Invalid(format!(
                "plugin {plugin_id} has no root_dir for config files"
            ))
        })
    }

    fn plugin_config_file_path(
        &self,
        plugin_id: &str,
        filename: &str,
    ) -> Result<PathBuf, ManagementPluginLifecycleError> {
        let root = self.plugin_root(plugin_id)?;
        Ok(root.join(filename))
    }
}

impl Default for ManagementPluginLifecycleState {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[derive(Clone, Debug)]
pub struct ManagementPluginSeed {
    pub source: PluginLoadSource,
    pub manifest: PluginManifest,
    pub state: PluginLifecycleState,
}

impl ManagementPluginSeed {
    pub fn new(
        source: PluginLoadSource,
        manifest: PluginManifest,
        state: PluginLifecycleState,
    ) -> Self {
        Self {
            source,
            manifest,
            state,
        }
    }
}

struct ManagementPluginLifecycleSnapshot {
    loader: PluginLoader<InMemoryPluginStore>,
    configs: BTreeMap<String, Value>,
    operations: Vec<ManagementPluginOperationRecord>,
    next_operation_seq: u64,
}

impl ManagementPluginLifecycleSnapshot {
    fn catalog_response(
        &self,
        handlers: PluginManagementResponse,
    ) -> ManagementPluginLifecycleCatalogResponse {
        let mut plugins = self
            .loader
            .store()
            .records()
            .iter()
            .map(|record| {
                ManagementPluginDescriptor::from_record(
                    record,
                    self.configs.get(record.plugin_id()).cloned(),
                )
            })
            .collect::<Vec<_>>();
        plugins.sort_by(|left, right| left.plugin_id.cmp(&right.plugin_id));

        ManagementPluginLifecycleCatalogResponse {
            handlers,
            plugins,
            operations: self.operations.clone(),
        }
    }

    fn record_operation(
        &mut self,
        plugin_id: String,
        action: String,
        message: String,
    ) -> ManagementPluginOperationRecord {
        let operation = ManagementPluginOperationRecord {
            operation_id: format!("plugin-op-{}", self.next_operation_seq),
            plugin_id,
            action,
            status: "completed".to_string(),
            message,
        };
        self.next_operation_seq += 1;
        self.operations.push(operation.clone());
        operation
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginLifecycleCatalogResponse {
    pub handlers: PluginManagementResponse,
    pub plugins: Vec<ManagementPluginDescriptor>,
    pub operations: Vec<ManagementPluginOperationRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementPluginDescriptor {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub state: String,
    pub active: bool,
    pub source: ManagementPluginSourceDescriptor,
    pub capabilities: Vec<String>,
    pub permissions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
    pub config_files: Vec<ManagementPluginConfigFileDescriptor>,
}

impl ManagementPluginDescriptor {
    fn from_record(record: &PluginRecord, config: Option<Value>) -> Self {
        Self {
            plugin_id: record.plugin_id().to_string(),
            name: record.metadata.manifest.name.clone(),
            version: record.metadata.manifest.version.clone(),
            description: record.metadata.manifest.description.clone(),
            state: lifecycle_state_label(record.state),
            active: record.is_active(),
            source: ManagementPluginSourceDescriptor::from_source(&record.metadata.source),
            capabilities: record
                .metadata
                .manifest
                .capabilities
                .iter()
                .map(|capability| format!("{capability:?}").to_ascii_lowercase())
                .collect(),
            permissions: record
                .metadata
                .manifest
                .permissions
                .iter()
                .map(|permission| format!("{permission:?}").to_ascii_lowercase())
                .collect(),
            config,
            config_files: record
                .metadata
                .source
                .root_dir()
                .map(|root| config_files_in_root(root))
                .unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPluginConfigFileDescriptor {
    pub filename: String,
    pub size_bytes: u64,
    pub modified_at_unix: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPluginSourceDescriptor {
    pub plugin_id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_path: Option<String>,
    pub reserved: bool,
}

impl ManagementPluginSourceDescriptor {
    fn from_source(source: &PluginLoadSource) -> Self {
        Self {
            plugin_id: source.plugin_id().to_string(),
            kind: source_kind_label(source.kind()),
            root_dir: source.root_dir().map(path_to_string),
            module_path: source.module_path().map(ToString::to_string),
            reserved: source.is_reserved(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPluginOperationRecord {
    pub operation_id: String,
    pub plugin_id: String,
    pub action: String,
    pub status: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPluginLifecycleEventDescriptor {
    pub plugin_id: String,
    pub action: String,
    pub previous: String,
    pub next: String,
}

impl ManagementPluginLifecycleEventDescriptor {
    fn from_event(event: PluginLifecycleEvent) -> Self {
        Self {
            plugin_id: event.plugin_id,
            action: lifecycle_action_label(event.action),
            previous: lifecycle_state_label(event.previous),
            next: lifecycle_state_label(event.next),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagementPluginAction {
    Load,
    Activate,
    Disable,
    Reload,
    Unload,
    Fail,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginLifecycleActionRequest {
    pub plugin_id: String,
    pub action: ManagementPluginAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginConfigRequest {
    pub plugin_id: String,
    #[serde(default)]
    pub config: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginConfigFileListRequest {
    pub plugin_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginConfigFileRequest {
    pub plugin_id: String,
    pub filename: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginConfigFileWriteRequest {
    pub plugin_id: String,
    pub filename: String,
    #[serde(default)]
    pub config: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginConfigFileListResponse {
    pub plugin_id: String,
    pub files: Vec<ManagementPluginConfigFileDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginConfigFileReadResponse {
    pub plugin_id: String,
    pub filename: String,
    pub config: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginConfigFileWriteResponse {
    pub plugin_id: String,
    pub file: ManagementPluginConfigFileDescriptor,
    pub config: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginConfigFileDeleteResponse {
    pub plugin_id: String,
    pub filename: String,
    pub deleted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginUploadPlanRequest {
    pub entries: Vec<String>,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPluginUploadPlanResponse {
    pub plugin_id: String,
    pub root_dir: String,
    pub entry_count: usize,
    pub overwrite: bool,
    pub requires_unpack: bool,
    pub accepted: bool,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginSourcePlanRequest {
    pub plugin_id: String,
    pub kind: String,
    #[serde(default)]
    pub root_dir: Option<String>,
    #[serde(default)]
    pub module_path: Option<String>,
    #[serde(default)]
    pub reserved: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginSourcePlanResponse {
    pub source: ManagementPluginSourceDescriptor,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyPluginNameRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub dir_name: String,
    #[serde(default)]
    pub delete_config: bool,
    #[serde(default)]
    pub delete_data: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPluginLifecycleMutationResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<ManagementPluginLifecycleEventDescriptor>,
    pub operation: ManagementPluginOperationRecord,
    pub catalog: ManagementPluginLifecycleCatalogResponse,
}

pub async fn lifecycle_catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementPluginLifecycleCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let lifecycle = state
        .plugin_lifecycle()
        .ok_or_else(plugin_lifecycle_unavailable)?;
    lifecycle
        .catalog_response(state.plugins().clone())
        .map(Json)
        .map_err(map_plugin_lifecycle_error)
}

pub async fn lifecycle_action(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPluginLifecycleActionRequest>,
) -> Result<Json<ManagementPluginLifecycleMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let lifecycle = state
        .plugin_lifecycle()
        .ok_or_else(plugin_lifecycle_unavailable)?;
    lifecycle
        .apply_action(request, state.plugins().clone())
        .map(Json)
        .map_err(map_plugin_lifecycle_error)
}

pub async fn upload_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPluginUploadPlanRequest>,
) -> Result<Json<ManagementPluginUploadPlanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let lifecycle = state
        .plugin_lifecycle()
        .ok_or_else(plugin_lifecycle_unavailable)?;
    lifecycle
        .upload_plan(request)
        .map(Json)
        .map_err(map_plugin_lifecycle_error)
}

pub async fn source_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPluginSourcePlanRequest>,
) -> Result<Json<ManagementPluginSourcePlanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let lifecycle = state
        .plugin_lifecycle()
        .ok_or_else(plugin_lifecycle_unavailable)?;
    lifecycle
        .source_plan(request)
        .map(Json)
        .map_err(map_plugin_lifecycle_error)
}

pub async fn save_config(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPluginConfigRequest>,
) -> Result<Json<ManagementPluginLifecycleMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let lifecycle = state
        .plugin_lifecycle()
        .ok_or_else(plugin_lifecycle_unavailable)?;
    lifecycle
        .save_config(request, state.plugins().clone())
        .map(Json)
        .map_err(map_plugin_lifecycle_error)
}

pub async fn list_config_files(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPluginConfigFileListRequest>,
) -> Result<Json<ManagementPluginConfigFileListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let lifecycle = state
        .plugin_lifecycle()
        .ok_or_else(plugin_lifecycle_unavailable)?;
    lifecycle
        .list_config_files(request)
        .map(Json)
        .map_err(map_plugin_lifecycle_error)
}

pub async fn read_config_file(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPluginConfigFileRequest>,
) -> Result<Json<ManagementPluginConfigFileReadResponse>, (StatusCode, Json<ErrorResponse>)> {
    let lifecycle = state
        .plugin_lifecycle()
        .ok_or_else(plugin_lifecycle_unavailable)?;
    lifecycle
        .read_config_file(request)
        .map(Json)
        .map_err(map_plugin_lifecycle_error)
}

pub async fn write_config_file(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPluginConfigFileWriteRequest>,
) -> Result<Json<ManagementPluginConfigFileWriteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let lifecycle = state
        .plugin_lifecycle()
        .ok_or_else(plugin_lifecycle_unavailable)?;
    lifecycle
        .write_config_file(request)
        .map(Json)
        .map_err(map_plugin_lifecycle_error)
}

pub async fn delete_config_file(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPluginConfigFileRequest>,
) -> Result<Json<ManagementPluginConfigFileDeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let lifecycle = state
        .plugin_lifecycle()
        .ok_or_else(plugin_lifecycle_unavailable)?;
    lifecycle
        .delete_config_file(request)
        .map(Json)
        .map_err(map_plugin_lifecycle_error)
}

pub async fn legacy_get(
    State(state): State<ManagementApiState>,
    Query(query): Query<BTreeMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let plugin_name = query.get("name").map(String::as_str);
    let plugins = if let Some(lifecycle) = state.plugin_lifecycle() {
        let catalog = lifecycle
            .catalog_response(state.plugins().clone())
            .map_err(map_plugin_lifecycle_error)?;
        catalog
            .plugins
            .iter()
            .filter(|plugin| {
                plugin_name.is_none_or(|name| plugin.plugin_id == name || plugin.name == name)
            })
            .map(legacy_plugin_value)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Ok(source_ok(json!(plugins)))
}

pub async fn legacy_on(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyPluginNameRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    legacy_lifecycle_action(state, request.plugin_id(), ManagementPluginAction::Activate).await
}

pub async fn legacy_off(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyPluginNameRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    legacy_lifecycle_action(state, request.plugin_id(), ManagementPluginAction::Disable).await
}

pub async fn legacy_reload(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyPluginNameRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    legacy_lifecycle_action(state, request.plugin_id(), ManagementPluginAction::Reload).await
}

pub async fn legacy_reload_failed(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyPluginNameRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    legacy_lifecycle_action(state, request.plugin_id(), ManagementPluginAction::Reload).await
}

pub async fn legacy_uninstall_failed(
    Json(request): Json<LegacyPluginNameRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    Ok(source_ok(json!({
        "dir_name": request.dir_name,
        "delete_config": request.delete_config,
        "delete_data": request.delete_data,
        "message": "failed plugin uninstall is capability-only in this facade",
    })))
}

pub async fn legacy_readme(Query(query): Query<BTreeMap<String, String>>) -> Json<Value> {
    let name = query.get("name").cloned().unwrap_or_default();
    source_ok(json!({
        "content": format!("# {name}\n\nREADME content is not available from the in-memory plugin lifecycle facade."),
    }))
}

pub async fn legacy_changelog(Query(query): Query<BTreeMap<String, String>>) -> Json<Value> {
    let name = query.get("name").cloned().unwrap_or_default();
    source_ok(json!({
        "content": format!("# {name} Changelog\n\nNo changelog file is attached to this plugin lifecycle facade."),
    }))
}

pub async fn legacy_source_get(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let sources = if let Some(lifecycle) = state.plugin_lifecycle() {
        lifecycle
            .catalog_response(state.plugins().clone())
            .map_err(map_plugin_lifecycle_error)?
            .plugins
            .into_iter()
            .map(|plugin| json!(plugin.source))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Ok(source_ok(json!(sources)))
}

pub async fn legacy_source_save(Json(payload): Json<Value>) -> Json<Value> {
    source_ok(json!({
        "saved": true,
        "sources": payload.get("sources").cloned().unwrap_or_else(|| json!([])),
        "message": "custom plugin sources are accepted but not persisted by this facade",
    }))
}

pub async fn legacy_failed_plugins(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let failed = if let Some(lifecycle) = state.plugin_lifecycle() {
        lifecycle
            .catalog_response(state.plugins().clone())
            .map_err(map_plugin_lifecycle_error)?
            .plugins
            .into_iter()
            .filter(|plugin| plugin.state == "failed")
            .map(|plugin| json!({ "name": plugin.name, "dir_name": plugin.plugin_id }))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Ok(source_ok(json!(failed)))
}

impl LegacyPluginNameRequest {
    fn plugin_id(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.trim().to_string();
        }
        self.dir_name.trim().to_string()
    }
}

async fn legacy_lifecycle_action(
    state: ManagementApiState,
    plugin_id: String,
    action: ManagementPluginAction,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let lifecycle = state
        .plugin_lifecycle()
        .ok_or_else(plugin_lifecycle_unavailable)?;
    let response = lifecycle
        .apply_action(
            ManagementPluginLifecycleActionRequest { plugin_id, action },
            state.plugins().clone(),
        )
        .map_err(map_plugin_lifecycle_error)?;
    Ok(source_ok(json!(response)))
}

fn legacy_plugin_value(plugin: &ManagementPluginDescriptor) -> Value {
    json!({
        "name": plugin.plugin_id,
        "display_name": plugin.name,
        "repo": plugin.source.root_dir.clone().unwrap_or_default(),
        "author": "",
        "desc": plugin.description.clone().unwrap_or_default(),
        "version": plugin.version,
        "reserved": plugin.source.reserved,
        "activated": plugin.active,
        "online_vesion": "",
        "handlers": [],
        "has_config": plugin.config.is_some() || !plugin.config_files.is_empty(),
        "config_files": plugin.config_files,
        "capabilities": plugin.capabilities,
        "permissions": plugin.permissions,
        "installed_at": Value::Null,
    })
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": "ok",
        "data": data,
    }))
}

fn plugin_upload_plan(
    request: ManagementPluginUploadPlanRequest,
) -> Result<ManagementPluginUploadPlanResponse, ManagementPluginLifecycleError> {
    let mut root_dir: Option<String> = None;
    let mut entry_count = 0;
    for entry in request
        .entries
        .iter()
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
    {
        if entry.starts_with('/') || entry.starts_with('\\') || entry.contains("..") {
            return Err(ManagementPluginLifecycleError::Invalid(format!(
                "invalid plugin archive entry {entry}"
            )));
        }
        let root = entry
            .split(['/', '\\'])
            .next()
            .filter(|segment| !segment.trim().is_empty())
            .ok_or_else(|| {
                ManagementPluginLifecycleError::Invalid("plugin archive entry is empty".to_string())
            })?
            .to_string();
        if let Some(known_root) = &root_dir {
            if known_root != &root {
                return Err(ManagementPluginLifecycleError::Invalid(
                    "plugin archive must contain a single top-level directory".to_string(),
                ));
            }
        } else {
            root_dir = Some(root);
        }
        entry_count += 1;
    }
    let root_dir = root_dir.ok_or_else(|| {
        ManagementPluginLifecycleError::Invalid("plugin upload plan requires entries".to_string())
    })?;
    let source = PluginLoadSource::python_compat(&root_dir);
    Ok(ManagementPluginUploadPlanResponse {
        plugin_id: source.plugin_id().to_string(),
        root_dir,
        entry_count,
        overwrite: request.overwrite,
        requires_unpack: true,
        accepted: true,
        message: "Plugin upload plan validates archive shape only; no file is unpacked."
            .to_string(),
    })
}

fn source_from_request(
    request: &ManagementPluginSourcePlanRequest,
) -> Result<PluginLoadSource, ManagementPluginLifecycleError> {
    let kind = match request.kind.trim() {
        "native_rust" => PluginLoadSourceKind::NativeRust,
        "python_compat" => PluginLoadSourceKind::PythonCompat,
        "wasm" => PluginLoadSourceKind::Wasm,
        "external_process" => PluginLoadSourceKind::ExternalProcess,
        other => {
            return Err(ManagementPluginLifecycleError::Invalid(format!(
                "unsupported plugin source kind {other}"
            )));
        }
    };
    let mut source = PluginLoadSource::new(kind, request.plugin_id.clone());
    if let Some(root_dir) = request
        .root_dir
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        source = source.with_root_dir(root_dir);
    }
    if let Some(module_path) = request
        .module_path
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        source = source.with_module_path(module_path);
    }
    if request.reserved {
        source = source.reserved();
    }
    Ok(source)
}

fn lifecycle_state_label(state: PluginLifecycleState) -> String {
    format!("{state:?}").to_ascii_lowercase()
}

fn lifecycle_action_label(action: PluginLifecycleAction) -> String {
    format!("{action:?}").to_ascii_lowercase()
}

fn source_kind_label(kind: PluginLoadSourceKind) -> String {
    match kind {
        PluginLoadSourceKind::NativeRust => "native_rust",
        PluginLoadSourceKind::PythonCompat => "python_compat",
        PluginLoadSourceKind::Wasm => "wasm",
        PluginLoadSourceKind::ExternalProcess => "external_process",
    }
    .to_string()
}

fn path_to_string(path: &PathBuf) -> String {
    path.display().to_string()
}

fn validate_config_filename(filename: &str) -> Result<String, ManagementPluginLifecycleError> {
    let trimmed = filename.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || !trimmed.ends_with(".json")
    {
        return Err(ManagementPluginLifecycleError::Invalid(
            "plugin config filename must be a direct .json file".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn config_files_in_root(root: &PathBuf) -> Vec<ManagementPluginConfigFileDescriptor> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut files = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let filename = entry.file_name().to_str()?.to_string();
            if path.is_file() && is_plugin_config_file(&filename) {
                config_file_descriptor(&path, filename).ok()
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.filename.cmp(&right.filename));
    files
}

fn is_plugin_config_file(filename: &str) -> bool {
    filename.ends_with(".json")
        || filename.ends_with(".toml")
        || filename.ends_with(".yaml")
        || filename.ends_with(".yml")
}

fn config_file_descriptor(
    path: &PathBuf,
    filename: String,
) -> Result<ManagementPluginConfigFileDescriptor, ManagementPluginLifecycleError> {
    let metadata = fs::metadata(path)
        .map_err(|error| ManagementPluginLifecycleError::Io(error.to_string()))?;
    Ok(ManagementPluginConfigFileDescriptor {
        filename,
        size_bytes: metadata.len(),
        modified_at_unix: metadata.modified().ok().and_then(system_time_unix_secs),
    })
}

fn system_time_unix_secs(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

#[derive(Debug)]
enum ManagementPluginLifecycleError {
    StateLock(String),
    Invalid(String),
    NotFound(String),
    Io(String),
}

fn plugin_lifecycle_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "plugin lifecycle state is not configured".to_string(),
        }),
    )
}

fn map_plugin_lifecycle_error(
    error: ManagementPluginLifecycleError,
) -> (StatusCode, Json<ErrorResponse>) {
    let (status, message) = match error {
        ManagementPluginLifecycleError::StateLock(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("plugin lifecycle state lock: {message}"),
        ),
        ManagementPluginLifecycleError::Invalid(message) => (StatusCode::BAD_REQUEST, message),
        ManagementPluginLifecycleError::NotFound(plugin_id) => (
            StatusCode::NOT_FOUND,
            format!("plugin {plugin_id} is not managed"),
        ),
        ManagementPluginLifecycleError::Io(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
    };

    (status, Json(ErrorResponse { error: message }))
}
