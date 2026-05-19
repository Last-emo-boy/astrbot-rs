use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    process::Command,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use astrbot_maintenance::{
    DashboardUpdatePlan, InMemoryMaintenanceOperationStore, LegacyPythonMigrationOptions,
    MaintenanceMigrationCheck, MaintenanceMigrationRequest, MaintenanceMigrationService,
    MaintenanceOperationId, MaintenanceOperationKind, MaintenanceOperationProgress,
    MaintenanceOperationStore, MaintenanceOperationSummary, MaintenancePackageInstallPlan,
    MaintenancePackageInstallRequest, PlannedMigrationService, PlannedReleaseUpdateService,
    ProjectUpdatePlan, ReleaseMetadata, ReleaseUpdateCheck, ReleaseUpdateService,
    run_legacy_python_migration,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone)]
pub struct ManagementMaintenanceState {
    operations: Arc<dyn MaintenanceOperationStore>,
    current_version: String,
    latest_version: Option<String>,
    dashboard_version: Option<String>,
    release_notes: Vec<ReleaseMetadata>,
    release_executor: Option<Arc<dyn MaintenanceReleaseExecutor>>,
    package_executor: Option<Arc<dyn MaintenancePackageExecutor>>,
    migration_executor: Option<Arc<dyn MaintenanceMigrationExecutor>>,
    restart_executor: Option<Arc<dyn MaintenanceRestartExecutor>>,
    migration_check: MaintenanceMigrationCheck,
}

impl std::fmt::Debug for ManagementMaintenanceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagementMaintenanceState")
            .field("current_version", &self.current_version)
            .field("latest_version", &self.latest_version)
            .field("dashboard_version", &self.dashboard_version)
            .field("release_notes", &self.release_notes)
            .field("has_release_executor", &self.release_executor.is_some())
            .field("has_package_executor", &self.package_executor.is_some())
            .field("has_migration_executor", &self.migration_executor.is_some())
            .field("migration_check", &self.migration_check)
            .finish_non_exhaustive()
    }
}

pub trait MaintenanceRestartExecutor: Send + Sync + std::fmt::Debug {
    fn restart(&self, request: &MaintenanceRestartRequest) -> Result<String, String>;
}

pub type MaintenanceExecutionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<String>, String>> + Send + 'a>>;

pub trait MaintenanceReleaseExecutor: Send + Sync + std::fmt::Debug {
    fn execute_project_update(&self, plan: ProjectUpdatePlan) -> MaintenanceExecutionFuture<'_>;

    fn execute_dashboard_update(&self, plan: DashboardUpdatePlan)
    -> MaintenanceExecutionFuture<'_>;
}

pub trait MaintenancePackageExecutor: Send + Sync + std::fmt::Debug {
    fn install_package(
        &self,
        plan: MaintenancePackageInstallPlan,
    ) -> MaintenanceExecutionFuture<'_>;
}

pub trait MaintenanceMigrationExecutor: Send + Sync + std::fmt::Debug {
    fn run_migration(&self, request: MaintenanceMigrationRequest)
    -> MaintenanceExecutionFuture<'_>;
}

#[derive(Clone, Debug)]
pub struct LocalMaintenanceExecutor {
    project_root: PathBuf,
    python: String,
    runtime_config_path: Option<PathBuf>,
    sqlite_path: Option<PathBuf>,
}

impl LocalMaintenanceExecutor {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            python: std::env::var("PYTHON").unwrap_or_else(|_| "python".to_string()),
            runtime_config_path: None,
            sqlite_path: None,
        }
    }

    pub fn with_python(mut self, python: impl Into<String>) -> Self {
        self.python = python.into();
        self
    }

    pub fn with_runtime_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.runtime_config_path = Some(path.into());
        self
    }

    pub fn with_sqlite_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.sqlite_path = Some(path.into());
        self
    }
}

impl MaintenanceReleaseExecutor for LocalMaintenanceExecutor {
    fn execute_project_update(&self, plan: ProjectUpdatePlan) -> MaintenanceExecutionFuture<'_> {
        let root = self.project_root.clone();
        let python = self.python.clone();
        Box::pin(async move { run_local_project_update(root, python, plan).await })
    }

    fn execute_dashboard_update(
        &self,
        plan: DashboardUpdatePlan,
    ) -> MaintenanceExecutionFuture<'_> {
        let root = self.project_root.clone();
        let python = self.python.clone();
        Box::pin(async move { run_local_dashboard_update(root, python, plan).await })
    }
}

impl MaintenancePackageExecutor for LocalMaintenanceExecutor {
    fn install_package(
        &self,
        plan: MaintenancePackageInstallPlan,
    ) -> MaintenanceExecutionFuture<'_> {
        let root = self.project_root.clone();
        let python = self.python.clone();
        Box::pin(async move { run_local_package_install(root, python, plan).await })
    }
}

impl MaintenanceMigrationExecutor for LocalMaintenanceExecutor {
    fn run_migration(
        &self,
        request: MaintenanceMigrationRequest,
    ) -> MaintenanceExecutionFuture<'_> {
        let runtime_config_path = self.runtime_config_path.clone();
        let sqlite_path = self.sqlite_path.clone();
        Box::pin(
            async move { run_local_migration(runtime_config_path, sqlite_path, request).await },
        )
    }
}

impl ManagementMaintenanceState {
    pub fn new(current_version: impl Into<String>) -> Self {
        Self {
            operations: Arc::new(InMemoryMaintenanceOperationStore::new()),
            current_version: current_version.into(),
            latest_version: None,
            dashboard_version: None,
            release_notes: Vec::new(),
            release_executor: None,
            package_executor: None,
            migration_executor: None,
            restart_executor: None,
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

    pub fn with_release_notes(mut self, release_notes: Vec<ReleaseMetadata>) -> Self {
        self.release_notes = release_notes;
        self
    }

    pub fn with_operation_store(mut self, operations: Arc<dyn MaintenanceOperationStore>) -> Self {
        self.operations = operations;
        self
    }

    pub fn with_release_executor(
        mut self,
        release_executor: Arc<dyn MaintenanceReleaseExecutor>,
    ) -> Self {
        self.release_executor = Some(release_executor);
        self
    }

    pub fn with_package_executor(
        mut self,
        package_executor: Arc<dyn MaintenancePackageExecutor>,
    ) -> Self {
        self.package_executor = Some(package_executor);
        self
    }

    pub fn with_migration_executor(
        mut self,
        migration_executor: Arc<dyn MaintenanceMigrationExecutor>,
    ) -> Self {
        self.migration_executor = Some(migration_executor);
        self
    }

    pub fn with_restart_executor(
        mut self,
        restart_executor: Arc<dyn MaintenanceRestartExecutor>,
    ) -> Self {
        self.restart_executor = Some(restart_executor);
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
            .with_pending_storage_migrations(
                self.migration_check.pending_storage_migrations.clone(),
            )
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

    pub async fn complete_operation(
        &self,
        operation_id: MaintenanceOperationId,
        confirmed: bool,
    ) -> astrbot_core::Result<Option<MaintenanceOperationSummary>> {
        let Some(mut operation) = self.operations.get_operation(&operation_id).await? else {
            return Ok(None);
        };
        if !confirmed {
            operation.progress = operation
                .progress
                .failed("maintenance operation requires explicit confirmation");
            self.operations.put_operation(operation.clone()).await?;
            return Ok(Some(operation));
        }

        operation.progress = match operation.kind {
            MaintenanceOperationKind::ProjectUpdate => {
                let executor = self.release_executor.as_ref().ok_or_else(|| {
                    astrbot_core::AstrbotError::Pipeline(
                        "maintenance release executor is not configured".to_string(),
                    )
                })?;
                let plan = operation_metadata::<ProjectUpdatePlan>(&operation, "project update")?;
                execute_progress(
                    "project update running",
                    "project update completed by dashboard executor",
                    executor.execute_project_update(plan).await,
                )
            }
            MaintenanceOperationKind::DashboardUpdate => {
                let executor = self.release_executor.as_ref().ok_or_else(|| {
                    astrbot_core::AstrbotError::Pipeline(
                        "maintenance release executor is not configured".to_string(),
                    )
                })?;
                let plan =
                    operation_metadata::<DashboardUpdatePlan>(&operation, "dashboard update")?;
                execute_progress(
                    "dashboard update running",
                    "dashboard update completed by dashboard executor",
                    executor.execute_dashboard_update(plan).await,
                )
            }
            MaintenanceOperationKind::PackageInstall => {
                let executor = self.package_executor.as_ref().ok_or_else(|| {
                    astrbot_core::AstrbotError::Pipeline(
                        "maintenance package executor is not configured".to_string(),
                    )
                })?;
                let plan = operation_metadata::<MaintenancePackageInstallPlan>(
                    &operation,
                    "package install",
                )?;
                execute_progress(
                    "package install running",
                    "package install completed by dashboard executor",
                    executor.install_package(plan).await,
                )
            }
            MaintenanceOperationKind::Migration => {
                let executor = self.migration_executor.as_ref().ok_or_else(|| {
                    astrbot_core::AstrbotError::Pipeline(
                        "maintenance migration executor is not configured".to_string(),
                    )
                })?;
                let request =
                    operation_metadata::<MaintenanceMigrationRequest>(&operation, "migration")?;
                execute_progress(
                    "migration running",
                    "migration completed by dashboard executor",
                    executor.run_migration(request).await,
                )
            }
            MaintenanceOperationKind::Restart => {
                let executor = self.restart_executor.as_ref().ok_or_else(|| {
                    astrbot_core::AstrbotError::Pipeline(
                        "maintenance restart executor is not configured".to_string(),
                    )
                })?;
                let request =
                    operation_metadata::<MaintenanceRestartRequest>(&operation, "restart")?;
                execute_restart_progress(executor.restart(&request))
            }
        };
        self.operations.put_operation(operation.clone()).await?;
        Ok(Some(operation))
    }

    pub async fn operations(&self) -> astrbot_core::Result<Vec<MaintenanceOperationSummary>> {
        self.operations.list_operations().await
    }

    pub fn changelog(&self) -> MaintenanceChangelogResponse {
        let releases = if self.release_notes.is_empty() {
            self.latest_version
                .iter()
                .map(|version| {
                    ReleaseMetadata::new(version.clone()).with_title(format!("AstrBot {version}"))
                })
                .collect()
        } else {
            self.release_notes.clone()
        };
        MaintenanceChangelogResponse {
            current_version: self.current_version.clone(),
            latest_version: self.latest_version.clone(),
            releases,
        }
    }

    pub async fn plan_restart(
        &self,
        request: MaintenanceRestartRequest,
    ) -> astrbot_core::Result<MaintenanceOperationSummary> {
        let operation = restart_operation(
            restart_operation_id(&request),
            MaintenanceOperationProgress::queued()
                .running("runtime restart planned")
                .running(restart_reason_message(&request)),
        )?
        .with_metadata(to_metadata(&request)?);
        self.operations.put_operation(operation.clone()).await?;
        Ok(operation)
    }

    pub async fn run_restart(
        &self,
        request: MaintenanceRestartRequest,
    ) -> astrbot_core::Result<MaintenanceOperationSummary> {
        let executor = self.restart_executor.as_ref().ok_or_else(|| {
            astrbot_core::AstrbotError::Pipeline(
                "maintenance restart executor is not configured".to_string(),
            )
        })?;
        let result = executor.restart(&request);
        let operation = restart_operation(
            restart_operation_id(&request),
            execute_restart_progress(result),
        )?
        .with_metadata(to_metadata(&request)?);
        self.operations.put_operation(operation.clone()).await?;
        Ok(operation)
    }

    pub async fn run_package_install(
        &self,
        request: MaintenancePackageInstallRequest,
    ) -> astrbot_core::Result<MaintenanceOperationSummary> {
        let plan = MaintenancePackageInstallPlan::global(request);
        let executor = self.package_executor.as_ref().ok_or_else(|| {
            astrbot_core::AstrbotError::Pipeline(
                "maintenance package executor is not configured".to_string(),
            )
        })?;
        let operation_id = package_operation_id(&plan);
        let operation = MaintenanceOperationSummary::new(
            operation_id,
            MaintenanceOperationKind::PackageInstall,
        )
        .with_progress(execute_progress(
            "package install running",
            "package install completed by dashboard executor",
            executor.install_package(plan.clone()).await,
        ))
        .with_metadata(to_metadata(&plan)?);
        self.operations.put_operation(operation.clone()).await?;
        Ok(operation)
    }

    pub async fn run_migration(
        &self,
        request: MaintenanceMigrationRequest,
    ) -> astrbot_core::Result<MaintenanceOperationSummary> {
        let operation_id = MaintenanceOperationId::new("migration")
            .expect("static migration operation id should be valid");
        let operation = if !request.confirmed {
            MaintenanceOperationSummary::new(operation_id, MaintenanceOperationKind::Migration)
                .with_progress(
                    MaintenanceOperationProgress::queued()
                        .failed("migration requires explicit confirmation"),
                )
                .with_metadata(to_metadata(&request)?)
        } else if let Some(executor) = &self.migration_executor {
            MaintenanceOperationSummary::new(operation_id, MaintenanceOperationKind::Migration)
                .with_progress(execute_progress(
                    "migration running",
                    "migration completed by dashboard executor",
                    executor.run_migration(request.clone()).await,
                ))
                .with_metadata(to_metadata(&request)?)
        } else {
            return self.migration_service().run_migration(request).await;
        };
        self.operations.put_operation(operation.clone()).await?;
        Ok(operation)
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
pub struct MaintenanceChangelogResponse {
    pub current_version: String,
    pub latest_version: Option<String>,
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
pub struct MaintenanceOperationRunRequest {
    pub operation_id: String,
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaintenanceOperationsResponse {
    pub operations: Vec<MaintenanceOperationSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaintenanceMigrationCheckResponse {
    pub check: MaintenanceMigrationCheck,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaintenanceRestartRequest {
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub delay_secs: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyUpdateCheckQuery {
    #[serde(default, rename = "type")]
    pub check_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyProjectUpdateRequest {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default = "default_true")]
    pub reboot: bool,
    #[serde(default)]
    pub proxy: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyMigrationRequest {
    #[serde(default)]
    pub platform_id_map:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyGhProxyRequest {
    pub proxy_url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyStatQuery {
    #[serde(default)]
    pub offset_sec: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyChangelogQuery {
    pub version: String,
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

pub async fn changelog(
    State(state): State<ManagementApiState>,
) -> Result<Json<MaintenanceChangelogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    Ok(Json(maintenance.changelog()))
}

pub async fn project_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<ProjectUpdatePlanRequest>,
) -> Result<Json<MaintenanceOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let mut plan = if request.latest
        || request
            .version
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
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
    let mut plan = if request.latest
        || request
            .version
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
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
    validate_package_request(&request)?;

    Ok(Json(MaintenancePackagePlanResponse {
        plan: MaintenancePackageInstallPlan::global(request),
    }))
}

pub async fn package_run(
    State(state): State<ManagementApiState>,
    Json(request): Json<MaintenancePackageInstallRequest>,
) -> Result<Json<MaintenanceOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_package_request(&request)?;
    if !request.confirmed {
        return Err(maintenance_bad_request(
            "package install requires explicit confirmation".to_string(),
        ));
    }
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let operation = maintenance
        .run_package_install(request)
        .await
        .map_err(map_maintenance_error)?;

    Ok(Json(MaintenanceOperationResponse { operation }))
}

pub async fn restart_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<MaintenanceRestartRequest>,
) -> Result<Json<MaintenanceOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let operation = maintenance
        .plan_restart(request)
        .await
        .map_err(map_maintenance_error)?;

    Ok(Json(MaintenanceOperationResponse { operation }))
}

pub async fn restart_run(
    State(state): State<ManagementApiState>,
    Json(request): Json<MaintenanceRestartRequest>,
) -> Result<Json<MaintenanceOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let operation = maintenance
        .run_restart(request)
        .await
        .map_err(map_restart_error)?;

    Ok(Json(MaintenanceOperationResponse { operation }))
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

pub async fn run_operation(
    State(state): State<ManagementApiState>,
    Json(request): Json<MaintenanceOperationRunRequest>,
) -> Result<Json<MaintenanceOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let operation_id = MaintenanceOperationId::new(request.operation_id)
        .ok_or_else(|| maintenance_bad_request("operation id is required".to_string()))?;
    let operation = maintenance
        .complete_operation(operation_id, request.confirmed)
        .await
        .map_err(map_maintenance_error)?
        .ok_or_else(|| maintenance_not_found("maintenance operation not found".to_string()))?;

    Ok(Json(MaintenanceOperationResponse { operation }))
}

pub async fn operation_catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<MaintenanceOperationsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let operations = maintenance
        .operations()
        .await
        .map_err(map_maintenance_error)?;

    Ok(Json(MaintenanceOperationsResponse { operations }))
}

pub async fn legacy_update_check(
    State(state): State<ManagementApiState>,
    Query(query): Query<LegacyUpdateCheckQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let check = maintenance
        .update_service()
        .check_updates()
        .await
        .map_err(map_maintenance_error)?;
    if query.check_type.as_deref() == Some("dashboard") {
        return Ok(source_ok(json!({
            "has_new_version": check.dashboard_has_new_version,
            "current_version": check.dashboard_version,
        })));
    }
    Ok(source_ok(release_check_to_source(&check)))
}

pub async fn legacy_update_releases(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let releases = maintenance
        .update_service()
        .releases()
        .await
        .map_err(map_maintenance_error)?;
    Ok(source_ok(json!(releases)))
}

pub async fn legacy_update_project(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyProjectUpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let mut plan = if request
        .version
        .as_deref()
        .is_none_or(|version| version.trim().is_empty() || version == "latest")
    {
        ProjectUpdatePlan::latest()
    } else {
        ProjectUpdatePlan::version(request.version.unwrap_or_default())
    }
    .with_reboot(request.reboot);
    if let Some(proxy) = request.proxy {
        plan = plan.with_proxy(proxy);
    }
    let planned = maintenance
        .update_service()
        .plan_project_update(plan)
        .await
        .map_err(map_maintenance_error)?;
    let operation = maintenance
        .complete_operation(planned.operation_id.clone(), true)
        .await
        .map_err(map_maintenance_error)?
        .unwrap_or(planned);
    Ok(source_ok(operation_to_source(operation)))
}

pub async fn legacy_update_dashboard(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let planned = maintenance
        .update_service()
        .plan_dashboard_update(DashboardUpdatePlan::for_runtime_version(
            maintenance.current_version.clone(),
        ))
        .await
        .map_err(map_maintenance_error)?;
    let operation = maintenance
        .complete_operation(planned.operation_id.clone(), true)
        .await
        .map_err(map_maintenance_error)?
        .unwrap_or(planned);
    Ok(source_ok(operation_to_source(operation)))
}

pub async fn legacy_update_package(
    State(state): State<ManagementApiState>,
    Json(mut request): Json<MaintenancePackageInstallRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    validate_package_request(&request)?;
    request.confirmed = true;
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let operation = maintenance
        .run_package_install(request)
        .await
        .map_err(map_maintenance_error)?;
    Ok(source_ok(operation_to_source(operation)))
}

pub async fn legacy_update_migration(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyMigrationRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let operation = maintenance
        .run_migration(MaintenanceMigrationRequest {
            confirmed: true,
            platform_id_map: request.platform_id_map,
        })
        .await
        .map_err(map_maintenance_error)?;
    Ok(source_ok(operation_to_source(operation)))
}

pub async fn legacy_stat_get(
    State(state): State<ManagementApiState>,
    Query(query): Query<LegacyStatQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let _ = query.offset_sec.unwrap_or(86_400);
    let observability = state.observability().ok_or_else(|| {
        map_maintenance_error(astrbot_core::AstrbotError::Pipeline(
            "management observability state is not configured".to_string(),
        ))
    })?;
    let metrics = observability
        .metrics()
        .map_err(|error| map_maintenance_error(astrbot_core::AstrbotError::Pipeline(error)))?;
    let persisted_platform_stats = observability
        .stats_since(None)
        .await
        .map_err(|error| map_maintenance_error(astrbot_core::AstrbotError::Pipeline(error)))?;
    let total_messages: i64 = metrics
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                astrbot_metrics::MetricEventKind::PlatformMessage
            )
        })
        .map(|event| event.count)
        .sum::<i64>()
        + persisted_platform_stats
            .iter()
            .map(|record| record.count)
            .sum::<i64>();
    let uptime = observability.uptime_seconds();
    let mut platform_counts = std::collections::BTreeMap::<String, i64>::new();
    for metric in metrics.iter().filter(|event| {
        matches!(
            event.kind,
            astrbot_metrics::MetricEventKind::PlatformMessage
        )
    }) {
        if let Some(platform_id) = &metric.platform_id {
            *platform_counts.entry(platform_id.clone()).or_default() += metric.count;
        }
    }
    for record in persisted_platform_stats {
        *platform_counts.entry(record.platform_id).or_default() += record.count;
    }
    Ok(source_ok(json!({
        "message_count": total_messages,
        "platform_count": state.platforms().platform_count,
        "plugin_count": state.plugins().handler_count,
        "platform": state.platforms().platform_ids.iter().map(|id| json!({ "name": id, "count": platform_counts.get(id).copied().unwrap_or(0) })).collect::<Vec<_>>(),
        "plugins": [],
        "message_time_series": [],
        "running": running_time_components(uptime),
        "memory": { "process": 0, "system": 0 },
        "cpu_percent": 0.0,
        "thread_count": 0,
        "start_time": current_unix().saturating_sub(uptime),
    })))
}

pub async fn legacy_stat_version(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let check = maintenance
        .update_service()
        .check_updates()
        .await
        .map_err(map_maintenance_error)?;
    let migration = maintenance
        .migration_service()
        .check_migration()
        .await
        .map_err(map_maintenance_error)?;
    Ok(source_ok(json!({
        "version": check.current_version,
        "dashboard_version": check.dashboard_version,
        "change_pwd_hint": false,
        "need_migration": migration.legacy_data_migration_needed || !migration.pending_storage_migrations.is_empty(),
    })))
}

pub async fn legacy_stat_start_time(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let uptime = state
        .observability()
        .map(|observability| observability.uptime_seconds())
        .unwrap_or(0);
    Ok(source_ok(
        json!({ "start_time": current_unix().saturating_sub(uptime) }),
    ))
}

pub async fn legacy_restart_core(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let operation = maintenance
        .run_restart(MaintenanceRestartRequest {
            reason: Some("source-compatible /api/stat/restart-core".to_string()),
            delay_secs: 0,
        })
        .await
        .map_err(map_restart_error)?;
    Ok(source_ok(operation_to_source(operation)))
}

pub async fn legacy_test_ghproxy_connection(
    Json(request): Json<LegacyGhProxyRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let proxy_url = request.proxy_url.trim().trim_end_matches('/');
    if proxy_url.is_empty() {
        return Err(maintenance_bad_request("proxy_url is required".to_string()));
    }
    if !(proxy_url.starts_with("http://") || proxy_url.starts_with("https://")) {
        return Err(maintenance_bad_request(
            "proxy_url must start with http:// or https://".to_string(),
        ));
    }
    let test_url = format!(
        "{proxy_url}/https://github.com/AstrBotDevs/AstrBot/raw/refs/heads/master/.python-version"
    );
    let started = Instant::now();
    let response = reqwest::Client::new()
        .get(test_url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|error| maintenance_bad_request(format!("Error: {error}")))?;
    if response.status().is_success() {
        Ok(source_ok(json!({
            "latency": started.elapsed().as_millis() as u64,
        })))
    } else {
        Err(maintenance_bad_request(format!(
            "Failed. Status code: {}",
            response.status().as_u16()
        )))
    }
}

pub async fn legacy_changelog(
    State(state): State<ManagementApiState>,
    Query(query): Query<LegacyChangelogQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let requested = normalize_version_label(&query.version);
    let changelog = maintenance.changelog();
    let release = changelog
        .releases
        .into_iter()
        .find(|release| normalize_version_label(&release.version) == requested)
        .ok_or_else(|| {
            maintenance_not_found(format!("Changelog for version {requested} not found"))
        })?;
    let content = release
        .notes
        .or(release.title)
        .unwrap_or_else(|| format!("AstrBot {}", release.version));
    Ok(source_ok(json!({
        "content": content,
        "version": requested,
    })))
}

pub async fn legacy_changelog_list(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let maintenance = state.maintenance().ok_or_else(maintenance_unavailable)?;
    let versions = maintenance
        .changelog()
        .releases
        .into_iter()
        .map(|release| normalize_version_label(&release.version))
        .collect::<Vec<_>>();
    Ok(source_ok(json!({ "versions": versions })))
}

pub async fn legacy_first_notice() -> Json<Value> {
    source_ok(json!({ "content": Value::Null }))
}

fn default_true() -> bool {
    true
}

fn release_check_to_source(check: &ReleaseUpdateCheck) -> Value {
    json!({
        "version": check.current_version,
        "current_version": check.current_version,
        "latest_version": check.latest_version,
        "has_new_version": check.has_new_version,
        "dashboard_version": check.dashboard_version,
        "dashboard_has_new_version": check.dashboard_has_new_version,
    })
}

fn operation_to_source(operation: MaintenanceOperationSummary) -> Value {
    json!({
        "operation": operation,
    })
}

fn running_time_components(total_seconds: u64) -> Value {
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    let hours = minutes / 60;
    json!({
        "hours": hours,
        "minutes": minutes % 60,
        "seconds": seconds,
    })
}

fn normalize_version_label(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}

fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": null,
        "data": data,
    }))
}

fn validate_package_request(
    request: &MaintenancePackageInstallRequest,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
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
    Ok(())
}

fn package_operation_id(plan: &MaintenancePackageInstallPlan) -> MaintenanceOperationId {
    let label = plan
        .request
        .package
        .as_deref()
        .or(plan.request.requirements_path.as_deref())
        .unwrap_or("requirements")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    MaintenanceOperationId::new(format!("package-install-{label}"))
        .expect("package operation id should be non-empty")
}

fn restart_operation_id(request: &MaintenanceRestartRequest) -> MaintenanceOperationId {
    let label = request
        .reason
        .as_deref()
        .unwrap_or("manual")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    MaintenanceOperationId::new(format!(
        "runtime-restart-{}",
        if label.is_empty() { "manual" } else { &label }
    ))
    .expect("restart operation id should be non-empty")
}

fn restart_operation(
    operation_id: MaintenanceOperationId,
    progress: MaintenanceOperationProgress,
) -> astrbot_core::Result<MaintenanceOperationSummary> {
    Ok(
        MaintenanceOperationSummary::new(operation_id, MaintenanceOperationKind::Restart)
            .with_progress(progress),
    )
}

fn restart_reason_message(request: &MaintenanceRestartRequest) -> String {
    let reason = request.reason.as_deref().unwrap_or("manual restart");
    if request.delay_secs > 0 {
        format!("restart after {}s: {reason}", request.delay_secs)
    } else {
        format!("restart immediately: {reason}")
    }
}

async fn run_local_project_update(
    root: PathBuf,
    python: String,
    plan: ProjectUpdatePlan,
) -> Result<Vec<String>, String> {
    let mut messages = Vec::new();
    if plan.latest {
        messages.push(
            run_command(
                root.clone(),
                "git".to_string(),
                vec!["pull".to_string(), "--ff-only".to_string()],
            )
            .await?,
        );
    } else if let Some(version) = plan
        .version
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        messages.push(
            run_command(
                root.clone(),
                "git".to_string(),
                vec!["fetch".to_string(), "--tags".to_string()],
            )
            .await?,
        );
        messages.push(
            run_command(
                root.clone(),
                "git".to_string(),
                vec!["checkout".to_string(), version.to_string()],
            )
            .await?,
        );
    }
    if plan.install_requirements {
        let requirements = root.join("requirements.txt");
        if requirements.is_file() {
            messages.push(
                run_command(
                    root.clone(),
                    python,
                    vec![
                        "-m".to_string(),
                        "pip".to_string(),
                        "install".to_string(),
                        "-r".to_string(),
                        requirements.to_string_lossy().to_string(),
                    ],
                )
                .await?,
            );
        } else {
            messages.push("requirements.txt not found; skipped package refresh".to_string());
        }
    }
    if plan.update_dashboard {
        messages.push(
            "dashboard update can be executed through dashboard-update operation".to_string(),
        );
    }
    Ok(messages)
}

async fn run_local_dashboard_update(
    root: PathBuf,
    python: String,
    plan: DashboardUpdatePlan,
) -> Result<Vec<String>, String> {
    let version = if plan.latest || plan.version.trim().is_empty() {
        "None".to_string()
    } else {
        serde_json::to_string(&plan.version).map_err(|error| error.to_string())?
    };
    let proxy = plan
        .proxy
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| "None".to_string());
    let code = format!(
        "import asyncio\nfrom astrbot.core.utils.io import download_dashboard\nasyncio.run(download_dashboard(latest={}, version={}, proxy={}))",
        if plan.latest { "True" } else { "False" },
        version,
        proxy
    );
    run_command(root, python, vec!["-c".to_string(), code])
        .await
        .map(|message| vec![message])
}

async fn run_local_package_install(
    root: PathBuf,
    python: String,
    plan: MaintenancePackageInstallPlan,
) -> Result<Vec<String>, String> {
    let mut args = vec!["-m".to_string(), "pip".to_string(), "install".to_string()];
    if let Some(package) = plan
        .request
        .package
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        args.push(package.to_string());
    } else if let Some(requirements_path) = plan
        .request
        .requirements_path
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        args.push("-r".to_string());
        args.push(requirements_path.to_string());
    } else {
        return Err("package or requirements_path is required".to_string());
    }
    if let Some(mirror) = plan
        .request
        .mirror
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        args.push("-i".to_string());
        args.push(mirror.to_string());
    }
    run_command(root, python, args)
        .await
        .map(|message| vec![message])
}

async fn run_local_migration(
    runtime_config_path: Option<PathBuf>,
    sqlite_path: Option<PathBuf>,
    request: MaintenanceMigrationRequest,
) -> Result<Vec<String>, String> {
    if !request.confirmed {
        return Err("migration requires explicit confirmation".to_string());
    }

    let mut messages = Vec::new();
    let Some(sqlite_path) = sqlite_path else {
        if let Some(config_path) = runtime_config_path {
            let display_path = config_path.display().to_string();
            messages.push(
                tokio::task::spawn_blocking(move || {
                    astrbot_runtime::RuntimeConfig::from_json_file(&config_path)
                        .map(|_| format!("runtime config defaults merged from {display_path}"))
                        .map_err(|error| error.to_string())
                })
                .await
                .map_err(|error| format!("runtime config migration task: {error}"))??,
            );
        } else {
            messages
                .push("runtime config migration skipped: no config path configured".to_string());
        }
        messages.push("sqlite storage migration skipped: no sqlite path configured".to_string());
        return Ok(messages);
    };

    let display_sqlite_path = sqlite_path.display().to_string();
    let mut options = LegacyPythonMigrationOptions::new(sqlite_path.clone())
        .with_platform_id_map(request.platform_id_map.clone());
    if let Some(config_path) = runtime_config_path.clone() {
        if let Some(data_dir) = config_path.parent() {
            options = options.with_legacy_data_dir(data_dir);
        }
        options = options.with_target_config_path(config_path);
    }
    let report = run_legacy_python_migration(options)
        .await
        .map_err(|error| error.to_string())?;
    if let Some(config_path) = runtime_config_path {
        messages.push(format!(
            "runtime config defaults merged from {}",
            config_path.display()
        ));
    } else {
        messages.push("runtime config migration skipped: no config path configured".to_string());
    }
    messages.push(format!(
        "sqlite storage migration applied for main_db v4 at {display_sqlite_path}"
    ));
    if let Some(report_path) = report.report_path {
        messages.push(format!(
            "legacy Python migration report written to {report_path}"
        ));
    }
    messages.push(format!(
        "legacy Python migration imported {} records and skipped {} records",
        report
            .tables
            .iter()
            .map(|table| table.imported)
            .sum::<usize>(),
        report
            .tables
            .iter()
            .map(|table| table.skipped)
            .sum::<usize>()
    ));

    if !request.platform_id_map.is_empty() {
        messages.push(format!(
            "legacy platform id map accepted for {} platform families",
            request.platform_id_map.len()
        ));
    }

    Ok(messages)
}

async fn run_command(cwd: PathBuf, program: String, args: Vec<String>) -> Result<String, String> {
    let display = format!("{} {}", program, args.join(" "));
    let blocking_display = display.clone();
    tokio::task::spawn_blocking(move || {
        Command::new(&program)
            .current_dir(cwd)
            .args(&args)
            .output()
            .map_err(|error| format!("{blocking_display}: {error}"))
    })
    .await
    .map_err(|error| format!("{display}: {error}"))?
    .and_then(|output| {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if output.status.success() {
            Ok(if stdout.is_empty() {
                format!("{display} completed")
            } else {
                stdout
            })
        } else {
            Err(if stderr.is_empty() {
                format!("{display} exited with {}", output.status)
            } else {
                stderr
            })
        }
    })
}

fn execute_progress(
    running_message: &str,
    completed_message: &str,
    result: Result<Vec<String>, String>,
) -> MaintenanceOperationProgress {
    let mut progress = MaintenanceOperationProgress::queued().running(running_message);
    match result {
        Ok(messages) => {
            for message in messages {
                if !message.trim().is_empty() {
                    progress = progress.running(message);
                }
            }
            progress.completed(completed_message)
        }
        Err(error) => progress.failed(error),
    }
}

fn execute_restart_progress(result: Result<String, String>) -> MaintenanceOperationProgress {
    match result {
        Ok(message) => MaintenanceOperationProgress::queued()
            .running("runtime restart requested")
            .completed(message),
        Err(error) => MaintenanceOperationProgress::queued()
            .running("runtime restart requested")
            .failed(error),
    }
}

fn operation_metadata<T>(
    operation: &MaintenanceOperationSummary,
    label: &str,
) -> astrbot_core::Result<T>
where
    T: DeserializeOwned,
{
    let metadata = operation.metadata.clone().ok_or_else(|| {
        astrbot_core::AstrbotError::Pipeline(format!("{label} operation metadata is missing"))
    })?;
    serde_json::from_value(metadata).map_err(|error| {
        astrbot_core::AstrbotError::Pipeline(format!(
            "{label} operation metadata is invalid: {error}"
        ))
    })
}

fn to_metadata<T: Serialize>(value: &T) -> astrbot_core::Result<serde_json::Value> {
    serde_json::to_value(value).map_err(|error| {
        astrbot_core::AstrbotError::Pipeline(format!("maintenance metadata: {error}"))
    })
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
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse { error: message }),
    )
}

fn maintenance_not_found(message: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse { error: message }),
    )
}

fn map_maintenance_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn map_restart_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    let message = error.to_string();
    if message.contains("restart executor is not configured") {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse { error: message }),
        )
    } else {
        map_maintenance_error(error)
    }
}
