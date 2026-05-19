use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use astrbot_skill::{
    SkillActivationChange, SkillActivationConfig, SkillCatalog, SkillDescriptor,
    SkillPackageDeletePlan, SkillPackageError, SkillPackageInstallPlan, SkillRuntimeInstallRequest,
    SkillRuntimeSnapshot, SkillSandboxCache,
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

#[derive(Clone, Debug)]
pub struct ManagementSkillState {
    inner: Arc<RwLock<ManagementSkillSnapshot>>,
}

impl ManagementSkillState {
    pub fn new(catalog: SkillCatalog) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ManagementSkillSnapshot {
                runtime: SkillRuntimeSnapshot::new(catalog),
            })),
        }
    }

    pub fn with_activation_config(self, activation: SkillActivationConfig) -> Self {
        if let Ok(mut snapshot) = self.inner.write() {
            snapshot.runtime.activation = activation;
        }
        self
    }

    pub fn with_sandbox_cache(self, sandbox_cache: SkillSandboxCache, exists: bool) -> Self {
        if let Ok(mut snapshot) = self.inner.write() {
            snapshot
                .runtime
                .replace_sandbox_cache(sandbox_cache, exists);
        }
        self
    }

    pub fn runtime_snapshot(&self) -> Result<SkillRuntimeSnapshot, String> {
        self.inner
            .read()
            .map_err(|error| format!("skill management state lock: {error}"))
            .map(|snapshot| snapshot.runtime.clone())
    }

    fn catalog_response(&self) -> Result<ManagementSkillCatalogResponse, String> {
        let snapshot = self
            .inner
            .read()
            .map_err(|error| format!("skill management state lock: {error}"))?;
        let catalog = snapshot.runtime.catalog_with_sandbox();
        let skills = catalog
            .skills()
            .iter()
            .map(|skill| {
                ManagementSkillDescriptor::from_descriptor(
                    skill,
                    skill.active && snapshot.runtime.activation.is_active(&skill.name),
                )
            })
            .collect();
        let sandbox_cache = snapshot
            .runtime
            .sandbox_cache
            .as_ref()
            .map(|cache| cache.status(snapshot.runtime.sandbox_cache_exists));

        Ok(ManagementSkillCatalogResponse {
            skills,
            sandbox_cache,
        })
    }

    fn set_active(
        &self,
        name: String,
        active: bool,
    ) -> Result<SkillActivationChange, SkillPackageError> {
        let mut snapshot = self.inner.write().map_err(|error| {
            SkillPackageError::invalid_skill_name(format!("skill state lock: {error}"))
        })?;
        snapshot.runtime.set_active(name, active)
    }

    fn delete_plan(&self, name: String) -> Result<SkillPackageDeletePlan, SkillPackageError> {
        let snapshot = self.inner.read().map_err(|error| {
            SkillPackageError::invalid_skill_name(format!("skill state lock: {error}"))
        })?;
        SkillPackageDeletePlan::from_catalog(&snapshot.runtime.catalog_with_sandbox(), name)
    }

    fn install(
        &self,
        request: ManagementSkillInstallPlanRequest,
    ) -> Result<ManagementSkillInstallResponse, SkillPackageError> {
        let entries = request.entries;
        let overwrite = request.overwrite;
        let plan = SkillPackageInstallPlan::from_zip_entries(entries.clone(), overwrite)?;
        let mut snapshot = self.inner.write().map_err(|error| {
            SkillPackageError::invalid_skill_name(format!("skill state lock: {error}"))
        })?;
        let outcome = snapshot
            .runtime
            .install_package(SkillRuntimeInstallRequest {
                entries,
                overwrite,
                description: Some("Installed from dashboard upload".to_string()),
                manifest_path: Some(format!("skills/{}/SKILL.md", plan.skill_name)),
            })?;

        Ok(ManagementSkillInstallResponse {
            plan: outcome.plan,
            skill: ManagementSkillDescriptor::from_descriptor(&outcome.skill, true),
        })
    }

    fn delete(&self, name: String) -> Result<ManagementSkillDeleteResponse, SkillPackageError> {
        let mut snapshot = self.inner.write().map_err(|error| {
            SkillPackageError::invalid_skill_name(format!("skill state lock: {error}"))
        })?;
        let outcome = snapshot.runtime.delete_package(name)?;
        let remaining_skill = outcome.remaining_skill.as_ref().map(|skill| {
            ManagementSkillDescriptor::from_descriptor(
                skill,
                skill.active && snapshot.runtime.activation.is_active(&skill.name),
            )
        });

        Ok(ManagementSkillDeleteResponse {
            plan: outcome.plan,
            deleted: outcome.deleted,
            remaining_skill,
        })
    }
}

#[derive(Clone, Debug)]
struct ManagementSkillSnapshot {
    runtime: SkillRuntimeSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementSkillCatalogResponse {
    pub skills: Vec<ManagementSkillDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_cache: Option<astrbot_skill::SkillSandboxCacheStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementSkillDescriptor {
    pub name: String,
    pub description: String,
    pub path: String,
    pub active: bool,
    pub source_type: String,
    pub source_label: String,
    pub local_exists: bool,
    pub sandbox_exists: bool,
}

impl ManagementSkillDescriptor {
    fn from_descriptor(descriptor: &SkillDescriptor, active: bool) -> Self {
        Self {
            name: descriptor.name.clone(),
            description: descriptor.description.clone(),
            path: descriptor.path.clone(),
            active,
            source_type: descriptor.source_type().to_string(),
            source_label: descriptor.source_label().to_string(),
            local_exists: descriptor.local_exists(),
            sandbox_exists: descriptor.sandbox_exists(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSkillActivationRequest {
    pub name: String,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSkillActivationResponse {
    pub change: SkillActivationChange,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSkillInstallPlanRequest {
    pub entries: Vec<String>,
    #[serde(default = "default_overwrite")]
    pub overwrite: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSkillInstallPlanResponse {
    pub plan: SkillPackageInstallPlan,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSkillInstallResponse {
    pub plan: SkillPackageInstallPlan,
    pub skill: ManagementSkillDescriptor,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSkillDeletePlanRequest {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSkillDeletePlanResponse {
    pub plan: SkillPackageDeletePlan,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSkillDeleteResponse {
    pub plan: SkillPackageDeletePlan,
    pub deleted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_skill: Option<ManagementSkillDescriptor>,
}

pub async fn catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementSkillCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let skills = state.skills().ok_or_else(skill_state_unavailable)?;
    skills
        .catalog_response()
        .map(Json)
        .map_err(skill_state_error)
}

pub async fn set_active(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSkillActivationRequest>,
) -> Result<Json<ManagementSkillActivationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let skills = state.skills().ok_or_else(skill_state_unavailable)?;
    let change = skills
        .set_active(request.name, request.active)
        .map_err(map_skill_package_error)?;

    Ok(Json(ManagementSkillActivationResponse { change }))
}

pub async fn install_plan(
    State(_state): State<ManagementApiState>,
    Json(request): Json<ManagementSkillInstallPlanRequest>,
) -> Result<Json<ManagementSkillInstallPlanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let plan = SkillPackageInstallPlan::from_zip_entries(request.entries, request.overwrite)
        .map_err(map_skill_package_error)?;

    Ok(Json(ManagementSkillInstallPlanResponse { plan }))
}

pub async fn install(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSkillInstallPlanRequest>,
) -> Result<Json<ManagementSkillInstallResponse>, (StatusCode, Json<ErrorResponse>)> {
    let skills = state.skills().ok_or_else(skill_state_unavailable)?;
    let response = skills.install(request).map_err(map_skill_package_error)?;

    Ok(Json(response))
}

pub async fn delete_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSkillDeletePlanRequest>,
) -> Result<Json<ManagementSkillDeletePlanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let skills = state.skills().ok_or_else(skill_state_unavailable)?;
    let plan = skills
        .delete_plan(request.name)
        .map_err(map_skill_package_error)?;

    Ok(Json(ManagementSkillDeletePlanResponse { plan }))
}

pub async fn delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSkillDeletePlanRequest>,
) -> Result<Json<ManagementSkillDeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let skills = state.skills().ok_or_else(skill_state_unavailable)?;
    let response = skills
        .delete(request.name)
        .map_err(map_skill_package_error)?;

    Ok(Json(response))
}

pub async fn legacy_catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let skills = state.skills().ok_or_else(skill_state_unavailable)?;
    let catalog = skills.catalog_response().map_err(skill_state_error)?;
    Ok(source_ok(json!({
        "skills": catalog.skills,
        "runtime": "local",
        "sandbox_cache": catalog.sandbox_cache,
    })))
}

pub async fn legacy_upload(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    legacy_install_from_payload(state, payload).await
}

pub async fn legacy_batch_upload(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let response = legacy_install_from_payload(state, payload).await?;
    Ok(source_ok(json!({
        "succeeded": [response.0["data"].clone()],
        "failed": [],
    })))
}

pub async fn legacy_update(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let skills = state.skills().ok_or_else(skill_state_unavailable)?;
    let name = payload
        .get("name")
        .or_else(|| payload.get("skill_name"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "skill name is required".to_string(),
                }),
            )
        })?;
    let active = payload
        .get("active")
        .and_then(Value::as_bool)
        .or_else(|| payload.get("enabled").and_then(Value::as_bool))
        .unwrap_or(true);
    let change = skills
        .set_active(name.to_string(), active)
        .map_err(map_skill_package_error)?;
    Ok(source_ok(json!({ "change": change })))
}

pub async fn legacy_delete(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let skills = state.skills().ok_or_else(skill_state_unavailable)?;
    let name = payload
        .get("name")
        .or_else(|| payload.get("skill_name"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "skill name is required".to_string(),
                }),
            )
        })?;
    let response = skills
        .delete(name.to_string())
        .map_err(map_skill_package_error)?;
    Ok(source_ok(json!(response)))
}

pub async fn legacy_download(
    Query(query): Query<BTreeMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    Ok(source_ok(json!({
        "name": query.get("name").cloned().unwrap_or_default(),
        "download_url": Value::Null,
        "message": "skill download is not backed by a file service in this management facade",
    })))
}

pub async fn legacy_neo_candidates() -> Json<Value> {
    source_ok(json!({
        "items": [],
        "configured": false,
        "message": "Shipyard Neo client is not configured in this Rust management facade",
    }))
}

pub async fn legacy_neo_releases() -> Json<Value> {
    source_ok(json!({
        "items": [],
        "configured": false,
        "message": "Shipyard Neo client is not configured in this Rust management facade",
    }))
}

pub async fn legacy_neo_payload(Query(query): Query<BTreeMap<String, String>>) -> Json<Value> {
    source_ok(json!({
        "candidate_id": query.get("candidate_id").cloned(),
        "release_id": query.get("release_id").cloned(),
        "payload": Value::Null,
        "configured": false,
    }))
}

pub async fn legacy_neo_action(Json(payload): Json<Value>) -> Json<Value> {
    source_ok(json!({
        "accepted": false,
        "payload": payload,
        "message": "Shipyard Neo mutation is capability-only until a Neo client is configured",
    }))
}

fn default_overwrite() -> bool {
    true
}

async fn legacy_install_from_payload(
    state: ManagementApiState,
    payload: Value,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let skills = state.skills().ok_or_else(skill_state_unavailable)?;
    let entries = skill_entries_from_payload(&payload)?;
    let overwrite = payload
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let response = skills
        .install(ManagementSkillInstallPlanRequest { entries, overwrite })
        .map_err(map_skill_package_error)?;
    Ok(source_ok(json!(response)))
}

fn skill_entries_from_payload(
    payload: &Value,
) -> Result<Vec<String>, (StatusCode, Json<ErrorResponse>)> {
    if let Some(entries) = payload.get("entries").and_then(Value::as_array) {
        let entries = entries
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if !entries.is_empty() {
            return Ok(entries);
        }
    }
    if let Some(name) = payload
        .get("name")
        .or_else(|| payload.get("skill_name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        return Ok(vec![format!("{name}/SKILL.md")]);
    }
    Err((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "skill upload requires entries or name".to_string(),
        }),
    ))
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": "ok",
        "data": data,
    }))
}

fn skill_state_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "skill management state is not configured".to_string(),
        }),
    )
}

fn skill_state_error(message: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: message }),
    )
}

fn map_skill_package_error(error: SkillPackageError) -> (StatusCode, Json<ErrorResponse>) {
    let status = match &error {
        SkillPackageError::SandboxOnlyMutation { .. } => StatusCode::FORBIDDEN,
        SkillPackageError::SkillAlreadyExists { .. } => StatusCode::CONFLICT,
        SkillPackageError::SkillNotFound { .. } => StatusCode::NOT_FOUND,
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}
