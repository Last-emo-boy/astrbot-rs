use std::sync::{Arc, RwLock};

use astrbot_skill::{
    SkillActivationChange, SkillActivationConfig, SkillCatalog, SkillDescriptor,
    SkillPackageDeletePlan, SkillPackageError, SkillPackageInstallPlan, SkillSandboxCache,
};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

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
                catalog,
                activation: SkillActivationConfig::new(),
                sandbox_cache: None,
                sandbox_cache_exists: false,
            })),
        }
    }

    pub fn with_activation_config(self, activation: SkillActivationConfig) -> Self {
        if let Ok(mut snapshot) = self.inner.write() {
            snapshot.activation = activation;
        }
        self
    }

    pub fn with_sandbox_cache(self, sandbox_cache: SkillSandboxCache, exists: bool) -> Self {
        if let Ok(mut snapshot) = self.inner.write() {
            snapshot.sandbox_cache = Some(sandbox_cache);
            snapshot.sandbox_cache_exists = exists;
        }
        self
    }

    fn catalog_response(&self) -> Result<ManagementSkillCatalogResponse, String> {
        let snapshot = self
            .inner
            .read()
            .map_err(|error| format!("skill management state lock: {error}"))?;
        let catalog = snapshot.catalog_with_sandbox();
        let skills = catalog
            .skills()
            .iter()
            .map(|skill| {
                ManagementSkillDescriptor::from_descriptor(
                    skill,
                    skill.active && snapshot.activation.is_active(&skill.name),
                )
            })
            .collect();
        let sandbox_cache = snapshot
            .sandbox_cache
            .as_ref()
            .map(|cache| cache.status(snapshot.sandbox_cache_exists));

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
        let catalog = snapshot.catalog_with_sandbox();
        snapshot.activation.set_active(&catalog, name, active)
    }

    fn delete_plan(&self, name: String) -> Result<SkillPackageDeletePlan, SkillPackageError> {
        let snapshot = self.inner.read().map_err(|error| {
            SkillPackageError::invalid_skill_name(format!("skill state lock: {error}"))
        })?;
        SkillPackageDeletePlan::from_catalog(&snapshot.catalog_with_sandbox(), name)
    }
}

#[derive(Clone, Debug)]
struct ManagementSkillSnapshot {
    catalog: SkillCatalog,
    activation: SkillActivationConfig,
    sandbox_cache: Option<SkillSandboxCache>,
    sandbox_cache_exists: bool,
}

impl ManagementSkillSnapshot {
    fn catalog_with_sandbox(&self) -> SkillCatalog {
        let mut catalog = self.catalog.clone();
        let Some(cache) = self.sandbox_cache.as_ref() else {
            return catalog;
        };

        for sandbox_skill in cache.as_descriptors() {
            if let Some(existing) = catalog.skill(&sandbox_skill.name).cloned() {
                catalog.add_skill(existing.with_source(astrbot_skill::SkillSource::Synced));
            } else {
                catalog.add_skill(sandbox_skill);
            }
        }

        catalog
    }
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
pub struct ManagementSkillDeletePlanRequest {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSkillDeletePlanResponse {
    pub plan: SkillPackageDeletePlan,
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

fn default_overwrite() -> bool {
    true
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
