use std::sync::Arc;

use astrbot_maintenance::{
    DashboardUpdatePlan, InMemoryMaintenanceOperationStore, MaintenanceMigrationCheck,
    MaintenanceMigrationRequest, MaintenanceMigrationService, MaintenanceOperationId,
    MaintenanceOperationStore, MaintenanceOperationSummary, MaintenancePackageInstallPlan,
    MaintenancePackageInstallRequest, PlannedMigrationService, PlannedReleaseUpdateService,
    ProjectUpdatePlan, ReleaseMetadata, ReleaseUpdateCheck, ReleaseUpdateService,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone)]
pub struct ManagementMaintenanceState {
    operations: Arc<dyn MaintenanceOperationStore>,
    current_version: String,
    latest_version: Option<String>,
    dashboard_version: Option<String>,
    migration_check: MaintenanceMigrationCheck,
}

impl std::fmt::Debug for ManagementMaintenanceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagementMaintenanceState")
            .field("current_version", &self.current_version)
            .field("latest_version", &self.latest_version)
            .field("dashboard_version", &self.dashboard_version)
            .field("migration_check", &self.migration_check)
            .finish_non_exhaustive()
    }
}

impl ManagementMaintenanceState {
    pub fn new(current_version: impl Into<String>) -> Self {
        Self {
            operations: Arc::new(InMemoryMaintenanceOperationStore::new()),
            current_version: current_version.into(),
            latest_version: None,
            dashboard_version: None,
            migration_check: MaintenanceMigrationCheck {
                runtime_config: astrbot_maintenance::RuntimeConfigMigrationDescriptor {
                    missing_default_keys: Vec::new(),
                },
                pending_storage_migrations: Vec::new(),
                legacy_data_migration_needed: false,
            },
        }
    }

    pub fn with_latest_version(mut self, latest_version: impl Into<String>) -> Self {
        let latest_version = latest_version.into();
        self.latest_version = (!latest_version.trim().is_empty()).then_some(latest_version);
        self
    }

    pub fn with_dashboard_version(mut self, dashboard_version: impl Into<String>) -> Self {
        let dashboard_version = dashboard_version.into();
        self.dashboard_version =
            (!dashboard_version.trim().is_empty()).then_some(dashboard_version);
        self
    }

    pub fn with_migration_check(mut self, migration_check: MaintenanceMigrationCheck) -> Self {
        self.migration_check = migration_check;
        self
    }

    fn update_service(&self) -> PlannedReleaseUpdateService<'_> {
        let mut service =
            PlannedReleaseUpdateService::new(self.operations.as_ref(), &self.current_version);
        if let Some(latest_version) = &self.latest_version {
            service = service.with_latest_version(latest_version);
        }
        if let Some(dashboard_version) = &self.dashboard_version {
            service = service.with_dashboard_version(dashboard_version);
        }
        service
    }

    fn migration_service(&self) -> PlannedMigrationService<'_> {
        PlannedMigrationService::new(self.operations.as_ref())
            .with_pending_storage_migrations(self.migration_check.pending_storage_migrations.clone())
            .with_legacy_data_migration_needed(self.migration_check.legacy_data_migration_needed)
            .with_runtime_config_plan(astrbot_runtime::RuntimeConfigMigrationPlan {
                missing_default_keys: self
                    .migration_check
                    .runtime_config
                    .missing_default_keys
                    .clone(),
            })
    }

    pub async fn operation(
        &self,
        operation_id: &MaintenanceOperationId,
    ) -> astrbot_core::Result<Option<MaintenanceOperationSummary>> {
        self.operations.get_operation(operation_id).await
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaintenanceCheckResponse {
    pub check: ReleaseUpdateCheck,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaintenanceReleasesResponse {
    pub releases: Vec<ReleaseMetadata>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectUpdatePlanRequest {
    pub version: Option<String>,
    #[serde(default)]
    pub latest: bool,
    pub proxy: Option<String>,
    #[serde(default = "default_true")]
    pub reboot: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardUpdatePlanRequest {
    pub version: Option<String>,
    #[serde(default)]
    pub latest: bool,
    pub proxy: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaintenanceOperationResponse {
    pub operation: MaintenanceOperationSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaintenancePackagePlanResponse {
    pub plan: MaintenancePackageInstallPlan,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaintenanceMigrationCheckResponse {
    pub check: MaintenanceMigrationCheck,
}

pub async fn check(
    State(state): State<ManagementApiState>,
) -> Result<Json<MaintenanceCheckResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let check = maintenance
        .update_service()
        .check_updates()
        .await
        .map_err(map_maintenance_error)?;

    Ok(Json(MaintenanceCheckResponse { check }))
}

pub async fn releases(
    State(state): State<ManagementApiState>,
) -> Result<Json<MaintenanceReleasesResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let releases = maintenance
        .update_service()
        .releases()
        .await
        .map_err(map_maintenance_error)?;

    Ok(Json(MaintenanceReleasesResponse { releases }))
}

pub async fn project_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<ProjectUpdatePlanRequest>,
) -> Result<Json<MaintenanceOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let mut plan = if request.latest || request.version.as_deref().unwrap_or_default().trim().is_empty() {
        ProjectUpdatePlan::latest()
    } else {
        ProjectUpdatePlan::version(request.version.unwrap_or_default())
    }
    .with_reboot(request.reboot);
    if let Some(proxy) = request.proxy {
        plan = plan.with_proxy(proxy);
    }
    let operation = maintenance
        .update_service()
        .plan_project_update(plan)
        .await
        .map_err(map_maintenance_error)?;

    Ok(Json(MaintenanceOperationResponse { operation }))
}

pub async fn dashboard_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<DashboardUpdatePlanRequest>,
) -> Result<Json<MaintenanceOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let mut plan = if request.latest || request.version.as_deref().unwrap_or_default().trim().is_empty() {
        DashboardUpdatePlan::latest()
    } else {
        DashboardUpdatePlan::for_runtime_version(request.version.unwrap_or_default())
    };
    if let Some(proxy) = request.proxy {
        let proxy = proxy.trim_end_matches('/').to_string();
        plan.proxy = (!proxy.trim().is_empty()).then_some(proxy);
    }
    let operation = maintenance
        .update_service()
        .plan_dashboard_update(plan)
        .await
        .map_err(map_maintenance_error)?;

    Ok(Json(MaintenanceOperationResponse { operation }))
}

pub async fn package_plan(
    Json(request): Json<MaintenancePackageInstallRequest>,
) -> Result<Json<MaintenancePackagePlanResponse>, (StatusCode, Json<ErrorResponse>)> {
    if request.package.as_deref().is_none_or(str::is_empty)
        && request
            .requirements_path
            .as_deref()
            .is_none_or(str::is_empty)
    {
        return Err(maintenance_bad_request(
            "package or requirements_path is required".to_string(),
        ));
    }

    Ok(Json(MaintenancePackagePlanResponse {
        plan: MaintenancePackageInstallPlan::global(request),
    }))
}

pub async fn migration_check(
    State(state): State<ManagementApiState>,
) -> Result<Json<MaintenanceMigrationCheckResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let check = maintenance
        .migration_service()
        .check_migration()
        .await
        .map_err(map_maintenance_error)?;

    Ok(Json(MaintenanceMigrationCheckResponse { check }))
}

pub async fn migration_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<MaintenanceMigrationRequest>,
) -> Result<Json<MaintenanceOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let operation = maintenance
        .migration_service()
        .run_migration(request)
        .await
        .map_err(map_maintenance_error)?;

    Ok(Json(MaintenanceOperationResponse { operation }))
}

pub async fn operation(
    State(state): State<ManagementApiState>,
    Path(operation_id): Path<String>,
) -> Result<Json<MaintenanceOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let operation_id = MaintenanceOperationId::new(operation_id)
        .ok_or_else(|| maintenance_bad_request("operation id is required".to_string()))?;
    let operation = maintenance
        .operation(&operation_id)
        .await
        .map_err(map_maintenance_error)?
        .ok_or_else(|| maintenance_not_found("maintenance operation not found".to_string()))?;

    Ok(Json(MaintenanceOperationResponse { operation }))
}

fn default_true() -> bool {
    true
}

fn maintenance_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "maintenance state is not configured".to_string(),
        }),
    )
}

fn maintenance_bad_request(message: String) -> (StatusCode, Json<ErrorResponse>) {
    (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: message }))
}

fn maintenance_not_found(message: String) -> (StatusCode, Json<ErrorResponse>) {
    (StatusCode::NOT_FOUND, Json(ErrorResponse { error: message }))
}

fn map_maintenance_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}
