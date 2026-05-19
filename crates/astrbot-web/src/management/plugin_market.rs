use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use astrbot_plugin::{
    PluginCompatibility, PluginInstallSource, PluginMarketAction, PluginMarketEntry,
    PluginMarketOperationPlan, PluginPackageDescriptor, PluginUninstallPlan, PluginUpdatePlan,
};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone, Debug)]
pub struct PluginMarketManagementState {
    inner: Arc<RwLock<PluginMarketSnapshot>>,
}

impl PluginMarketManagementState {
    pub fn new(entries: Vec<PluginMarketEntry>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(PluginMarketSnapshot {
                entries,
                installed: HashMap::new(),
                operations: Vec::new(),
                next_operation_seq: 1,
            })),
        }
    }

    fn catalog_response(&self) -> Result<PluginMarketCatalogResponse, PluginMarketError> {
        let snapshot = self.read_snapshot()?;
        Ok(snapshot.catalog_response())
    }

    fn install_plan(
        &self,
        plugin_id: &str,
    ) -> Result<PluginMarketOperationPlan, PluginMarketError> {
        let snapshot = self.read_snapshot()?;
        let entry = snapshot
            .entry(plugin_id)
            .ok_or_else(|| PluginMarketError::PluginNotFound {
                plugin_id: plugin_id.to_string(),
            })?;
        PluginMarketOperationPlan::from_market_entry(entry).ok_or_else(|| {
            PluginMarketError::MissingInstallSource {
                plugin_id: plugin_id.to_string(),
            }
        })
    }

    fn update_plan(&self, plugin_id: &str) -> Result<PluginMarketOperationPlan, PluginMarketError> {
        let snapshot = self.read_snapshot()?;
        let entry = snapshot
            .entry(plugin_id)
            .ok_or_else(|| PluginMarketError::PluginNotFound {
                plugin_id: plugin_id.to_string(),
            })?;
        let package =
            package_for_entry(entry).ok_or_else(|| PluginMarketError::MissingUpdateSource {
                plugin_id: plugin_id.to_string(),
            })?;

        Ok(PluginMarketOperationPlan::update(
            PluginUpdatePlan::new(entry.plugin_id.clone(), package)
                .with_compatibility(entry.compatibility.clone()),
        ))
    }

    fn uninstall_plan(&self, request: PluginMarketPlanRequest) -> PluginMarketOperationPlan {
        PluginMarketOperationPlan::uninstall(uninstall_plan_from_request(request))
    }

    fn execute_install(
        &self,
        request: PluginMarketPlanRequest,
    ) -> Result<PluginMarketExecuteResponse, PluginMarketError> {
        let mut snapshot = self.write_snapshot()?;
        let entry = snapshot.entry(&request.plugin_id).cloned().ok_or_else(|| {
            PluginMarketError::PluginNotFound {
                plugin_id: request.plugin_id.clone(),
            }
        })?;
        let plan = PluginMarketOperationPlan::from_market_entry(&entry).ok_or_else(|| {
            PluginMarketError::MissingInstallSource {
                plugin_id: request.plugin_id.clone(),
            }
        })?;
        ensure_compatible(&plan, request.ignore_compatibility)?;

        snapshot.installed.insert(
            entry.plugin_id.clone(),
            PluginMarketInstalledPlugin::from_entry(&entry, &plan),
        );
        let operation = snapshot.record_operation(plan.clone(), "plugin installed");

        Ok(snapshot.execute_response(plan, operation))
    }

    fn execute_update(
        &self,
        request: PluginMarketPlanRequest,
    ) -> Result<PluginMarketExecuteResponse, PluginMarketError> {
        let mut snapshot = self.write_snapshot()?;
        if !snapshot.installed.contains_key(&request.plugin_id) {
            return Err(PluginMarketError::PluginNotInstalled {
                plugin_id: request.plugin_id,
            });
        }
        let entry = snapshot.entry(&request.plugin_id).cloned().ok_or_else(|| {
            PluginMarketError::PluginNotFound {
                plugin_id: request.plugin_id.clone(),
            }
        })?;
        let package =
            package_for_entry(&entry).ok_or_else(|| PluginMarketError::MissingUpdateSource {
                plugin_id: request.plugin_id.clone(),
            })?;
        let plan = PluginMarketOperationPlan::update(
            PluginUpdatePlan::new(entry.plugin_id.clone(), package)
                .with_compatibility(entry.compatibility.clone()),
        );
        ensure_compatible(&plan, request.ignore_compatibility)?;

        snapshot.installed.insert(
            entry.plugin_id.clone(),
            PluginMarketInstalledPlugin::from_entry(&entry, &plan),
        );
        let operation = snapshot.record_operation(plan.clone(), "plugin updated");

        Ok(snapshot.execute_response(plan, operation))
    }

    fn execute_uninstall(
        &self,
        request: PluginMarketPlanRequest,
    ) -> Result<PluginMarketExecuteResponse, PluginMarketError> {
        let mut snapshot = self.write_snapshot()?;
        if !snapshot.installed.contains_key(&request.plugin_id) {
            return Err(PluginMarketError::PluginNotInstalled {
                plugin_id: request.plugin_id,
            });
        }
        let plan = PluginMarketOperationPlan::uninstall(uninstall_plan_from_request(request));
        snapshot.installed.remove(&plan.plugin_id);
        let operation = snapshot.record_operation(plan.clone(), "plugin uninstalled");

        Ok(snapshot.execute_response(plan, operation))
    }

    fn update_all_plan(&self) -> Result<PluginMarketUpdateAllPlanResponse, PluginMarketError> {
        let snapshot = self.read_snapshot()?;
        Ok(snapshot.update_all_plan_response())
    }

    fn execute_update_all(
        &self,
        request: PluginMarketUpdateAllRequest,
    ) -> Result<PluginMarketUpdateAllExecuteResponse, PluginMarketError> {
        let mut snapshot = self.write_snapshot()?;
        let plan_response = snapshot.update_all_plan_response();
        let mut operations = Vec::new();

        for plan in plan_response.plans.iter().cloned() {
            ensure_compatible(&plan, request.ignore_compatibility)?;
            let Some(entry) = snapshot.entry(&plan.plugin_id).cloned() else {
                continue;
            };
            snapshot.installed.insert(
                entry.plugin_id.clone(),
                PluginMarketInstalledPlugin::from_entry(&entry, &plan),
            );
            operations.push(snapshot.record_operation(plan, "plugin updated by update-all"));
        }

        Ok(PluginMarketUpdateAllExecuteResponse {
            plans: plan_response.plans,
            skipped_plugins: plan_response.skipped_plugins,
            operations,
            installed_plugins: snapshot.installed_plugins(),
        })
    }

    fn read_snapshot(
        &self,
    ) -> Result<std::sync::RwLockReadGuard<'_, PluginMarketSnapshot>, PluginMarketError> {
        self.inner
            .read()
            .map_err(|error| PluginMarketError::StateLock(error.to_string()))
    }

    fn write_snapshot(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, PluginMarketSnapshot>, PluginMarketError> {
        self.inner
            .write()
            .map_err(|error| PluginMarketError::StateLock(error.to_string()))
    }
}

#[derive(Clone, Debug)]
struct PluginMarketSnapshot {
    entries: Vec<PluginMarketEntry>,
    installed: HashMap<String, PluginMarketInstalledPlugin>,
    operations: Vec<PluginMarketOperationRecord>,
    next_operation_seq: u64,
}

impl PluginMarketSnapshot {
    fn entry(&self, plugin_id: &str) -> Option<&PluginMarketEntry> {
        self.entries
            .iter()
            .find(|entry| entry.plugin_id == plugin_id)
    }

    fn catalog_response(&self) -> PluginMarketCatalogResponse {
        PluginMarketCatalogResponse {
            plugins: self
                .entries
                .iter()
                .map(|entry| {
                    PluginMarketPluginDescriptor::from_entry(
                        entry,
                        self.installed.get(&entry.plugin_id),
                    )
                })
                .collect(),
            installed_plugins: self.installed_plugins(),
            operations: self.operations.clone(),
        }
    }

    fn installed_plugins(&self) -> Vec<PluginMarketInstalledPlugin> {
        let mut installed = self.installed.values().cloned().collect::<Vec<_>>();
        installed.sort_by(|left, right| left.plugin_id.cmp(&right.plugin_id));
        installed
    }

    fn record_operation(
        &mut self,
        plan: PluginMarketOperationPlan,
        message: impl Into<String>,
    ) -> PluginMarketOperationRecord {
        let operation = PluginMarketOperationRecord {
            operation_id: format!("plugin-market-op-{}", self.next_operation_seq),
            plugin_id: plan.plugin_id.clone(),
            action: plan.action,
            status: PluginMarketOperationStatus::Completed,
            message: message.into(),
            plan,
        };
        self.next_operation_seq += 1;
        self.operations.push(operation.clone());
        operation
    }

    fn execute_response(
        &self,
        plan: PluginMarketOperationPlan,
        operation: PluginMarketOperationRecord,
    ) -> PluginMarketExecuteResponse {
        PluginMarketExecuteResponse {
            plan,
            operation,
            installed_plugins: self.installed_plugins(),
        }
    }

    fn update_all_plan_response(&self) -> PluginMarketUpdateAllPlanResponse {
        let mut plans = Vec::new();
        let mut skipped_plugins = Vec::new();
        for plugin_id in self.installed.keys() {
            let Some(entry) = self.entry(plugin_id) else {
                skipped_plugins.push(format!("{plugin_id}: missing market entry"));
                continue;
            };
            let Some(package) = package_for_entry(entry) else {
                skipped_plugins.push(format!("{plugin_id}: missing update source"));
                continue;
            };
            plans.push(PluginMarketOperationPlan::update(
                PluginUpdatePlan::new(entry.plugin_id.clone(), package)
                    .with_compatibility(entry.compatibility.clone()),
            ));
        }
        plans.sort_by(|left, right| left.plugin_id.cmp(&right.plugin_id));
        skipped_plugins.sort();
        PluginMarketUpdateAllPlanResponse {
            plans,
            skipped_plugins,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginMarketCatalogResponse {
    pub plugins: Vec<PluginMarketPluginDescriptor>,
    pub installed_plugins: Vec<PluginMarketInstalledPlugin>,
    pub operations: Vec<PluginMarketOperationRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMarketPluginDescriptor {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<PluginPackageDescriptor>,
    pub compatibility: PluginCompatibility,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme: Option<astrbot_plugin::PluginDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changelog: Option<astrbot_plugin::PluginDocument>,
    pub installed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_version: Option<String>,
    pub pending_loader_reload: bool,
}

impl PluginMarketPluginDescriptor {
    fn from_entry(
        entry: &PluginMarketEntry,
        installed: Option<&PluginMarketInstalledPlugin>,
    ) -> Self {
        Self {
            plugin_id: entry.plugin_id.clone(),
            name: entry.name.clone(),
            version: entry.version.clone(),
            repo_url: entry.repo_url.clone(),
            package: entry.package.clone(),
            compatibility: entry.compatibility.clone(),
            readme: entry.readme.clone(),
            changelog: entry.changelog.clone(),
            installed: installed.is_some(),
            installed_version: installed.map(|plugin| plugin.version.clone()),
            pending_loader_reload: installed
                .map(|plugin| plugin.pending_loader_reload)
                .unwrap_or(false),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMarketInstalledPlugin {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<PluginPackageDescriptor>,
    pub compatibility: PluginCompatibility,
    pub pending_loader_reload: bool,
}

impl PluginMarketInstalledPlugin {
    fn from_entry(entry: &PluginMarketEntry, plan: &PluginMarketOperationPlan) -> Self {
        Self {
            plugin_id: entry.plugin_id.clone(),
            name: entry.name.clone(),
            version: entry.version.clone(),
            package: plan.package.clone(),
            compatibility: entry.compatibility.clone(),
            pending_loader_reload: plan.requires_loader_reload,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMarketOperationRecord {
    pub operation_id: String,
    pub plugin_id: String,
    pub action: PluginMarketAction,
    pub status: PluginMarketOperationStatus,
    pub message: String,
    pub plan: PluginMarketOperationPlan,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginMarketOperationStatus {
    Completed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginMarketPlanRequest {
    pub plugin_id: String,
    #[serde(default)]
    pub delete_config: bool,
    #[serde(default)]
    pub delete_data: bool,
    #[serde(default)]
    pub ignore_compatibility: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginMarketPlanResponse {
    pub plan: PluginMarketOperationPlan,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginMarketExecuteResponse {
    pub plan: PluginMarketOperationPlan,
    pub operation: PluginMarketOperationRecord,
    pub installed_plugins: Vec<PluginMarketInstalledPlugin>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginMarketUpdateAllRequest {
    #[serde(default)]
    pub ignore_compatibility: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginMarketUpdateAllPlanResponse {
    pub plans: Vec<PluginMarketOperationPlan>,
    pub skipped_plugins: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginMarketUpdateAllExecuteResponse {
    pub plans: Vec<PluginMarketOperationPlan>,
    pub skipped_plugins: Vec<String>,
    pub operations: Vec<PluginMarketOperationRecord>,
    pub installed_plugins: Vec<PluginMarketInstalledPlugin>,
}

pub async fn catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<PluginMarketCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    market
        .catalog_response()
        .map(Json)
        .map_err(map_plugin_market_error)
}

pub async fn install_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<PluginMarketPlanRequest>,
) -> Result<Json<PluginMarketPlanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    let plan = market
        .install_plan(&request.plugin_id)
        .map_err(map_plugin_market_error)?;

    Ok(Json(PluginMarketPlanResponse { plan }))
}

pub async fn update_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<PluginMarketPlanRequest>,
) -> Result<Json<PluginMarketPlanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    let plan = market
        .update_plan(&request.plugin_id)
        .map_err(map_plugin_market_error)?;

    Ok(Json(PluginMarketPlanResponse { plan }))
}

pub async fn uninstall_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<PluginMarketPlanRequest>,
) -> Result<Json<PluginMarketPlanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    let plan = market.uninstall_plan(request);

    Ok(Json(PluginMarketPlanResponse { plan }))
}

pub async fn install(
    State(state): State<ManagementApiState>,
    Json(request): Json<PluginMarketPlanRequest>,
) -> Result<Json<PluginMarketExecuteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    market
        .execute_install(request)
        .map(Json)
        .map_err(map_plugin_market_error)
}

pub async fn update(
    State(state): State<ManagementApiState>,
    Json(request): Json<PluginMarketPlanRequest>,
) -> Result<Json<PluginMarketExecuteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    market
        .execute_update(request)
        .map(Json)
        .map_err(map_plugin_market_error)
}

pub async fn uninstall(
    State(state): State<ManagementApiState>,
    Json(request): Json<PluginMarketPlanRequest>,
) -> Result<Json<PluginMarketExecuteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    market
        .execute_uninstall(request)
        .map(Json)
        .map_err(map_plugin_market_error)
}

pub async fn update_all_plan(
    State(state): State<ManagementApiState>,
) -> Result<Json<PluginMarketUpdateAllPlanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    market
        .update_all_plan()
        .map(Json)
        .map_err(map_plugin_market_error)
}

pub async fn update_all(
    State(state): State<ManagementApiState>,
    Json(request): Json<PluginMarketUpdateAllRequest>,
) -> Result<Json<PluginMarketUpdateAllExecuteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    market
        .execute_update_all(request)
        .map(Json)
        .map_err(map_plugin_market_error)
}

pub async fn legacy_market_list(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    let catalog = market.catalog_response().map_err(map_plugin_market_error)?;
    Ok(source_ok(json!(catalog.plugins)))
}

pub async fn legacy_install(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    let request = legacy_plan_request(payload)?;
    let response = market
        .execute_install(request)
        .map_err(map_plugin_market_error)?;
    Ok(source_ok(json!(response)))
}

pub async fn legacy_update(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    let request = legacy_plan_request(payload)?;
    let response = market
        .execute_update(request)
        .map_err(map_plugin_market_error)?;
    Ok(source_ok(json!(response)))
}

pub async fn legacy_update_all(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    let request = PluginMarketUpdateAllRequest {
        ignore_compatibility: payload
            .get("ignore_compatibility")
            .or_else(|| payload.get("ignore_version_check"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    let response = market
        .execute_update_all(request)
        .map_err(map_plugin_market_error)?;
    Ok(source_ok(json!(response)))
}

pub async fn legacy_uninstall(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    let request = legacy_plan_request(payload)?;
    let response = market
        .execute_uninstall(request)
        .map_err(map_plugin_market_error)?;
    Ok(source_ok(json!(response)))
}

pub async fn legacy_check_compat(Json(payload): Json<Value>) -> Json<Value> {
    let version = payload
        .get("astrbot_version")
        .or_else(|| payload.get("version"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    source_ok(json!({
        "compatible": true,
        "message": "compatibility check is accepted by the Rust management facade",
        "astrbot_version": version,
    }))
}

fn package_for_entry(entry: &PluginMarketEntry) -> Option<PluginPackageDescriptor> {
    entry.package.clone().or_else(|| {
        entry
            .repo_url
            .as_ref()
            .map(|url| PluginPackageDescriptor::new(PluginInstallSource::repository(url.as_str())))
    })
}

fn legacy_plan_request(
    payload: Value,
) -> Result<PluginMarketPlanRequest, (StatusCode, Json<ErrorResponse>)> {
    let plugin_id = payload
        .get("plugin_id")
        .or_else(|| payload.get("name"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            payload
                .get("url")
                .and_then(Value::as_str)
                .map(plugin_id_from_url)
        })
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "plugin_id, name, or url is required".to_string(),
                }),
            )
        })?;
    Ok(PluginMarketPlanRequest {
        plugin_id,
        delete_config: payload
            .get("delete_config")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        delete_data: payload
            .get("delete_data")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        ignore_compatibility: payload
            .get("ignore_compatibility")
            .or_else(|| payload.get("ignore_version_check"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn plugin_id_from_url(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(url)
        .trim_end_matches(".git")
        .trim_end_matches(".zip")
        .replace('-', "_")
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": "ok",
        "data": data,
    }))
}

fn uninstall_plan_from_request(request: PluginMarketPlanRequest) -> PluginUninstallPlan {
    let mut plan = PluginUninstallPlan::new(request.plugin_id);
    if request.delete_config {
        plan = plan.delete_config();
    }
    if request.delete_data {
        plan = plan.delete_data();
    }
    plan
}

fn ensure_compatible(
    plan: &PluginMarketOperationPlan,
    ignore: bool,
) -> Result<(), PluginMarketError> {
    if plan.is_blocked_by_compatibility() && !ignore {
        return Err(PluginMarketError::Incompatible {
            plugin_id: plan.plugin_id.clone(),
            message: plan.compatibility.message.clone().unwrap_or_else(|| {
                "plugin is incompatible with current AstrBot version".to_string()
            }),
        });
    }
    Ok(())
}

fn plugin_market_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "plugin market state is not configured".to_string(),
        }),
    )
}

#[derive(Debug)]
enum PluginMarketError {
    StateLock(String),
    PluginNotFound { plugin_id: String },
    PluginNotInstalled { plugin_id: String },
    MissingInstallSource { plugin_id: String },
    MissingUpdateSource { plugin_id: String },
    Incompatible { plugin_id: String, message: String },
}

fn map_plugin_market_error(error: PluginMarketError) -> (StatusCode, Json<ErrorResponse>) {
    let (status, message) = match error {
        PluginMarketError::StateLock(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("plugin market state lock: {message}"),
        ),
        PluginMarketError::PluginNotFound { plugin_id } => (
            StatusCode::NOT_FOUND,
            format!("plugin {plugin_id} is not in market"),
        ),
        PluginMarketError::PluginNotInstalled { plugin_id } => (
            StatusCode::CONFLICT,
            format!("plugin {plugin_id} is not installed"),
        ),
        PluginMarketError::MissingInstallSource { plugin_id } => (
            StatusCode::BAD_REQUEST,
            format!("plugin {plugin_id} has no install source"),
        ),
        PluginMarketError::MissingUpdateSource { plugin_id } => (
            StatusCode::BAD_REQUEST,
            format!("plugin {plugin_id} has no update source"),
        ),
        PluginMarketError::Incompatible { plugin_id, message } => (
            StatusCode::CONFLICT,
            format!("plugin {plugin_id} is incompatible: {message}"),
        ),
    };

    (status, Json(ErrorResponse { error: message }))
}
