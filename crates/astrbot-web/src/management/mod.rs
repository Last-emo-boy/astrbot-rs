mod api_key;
mod auth;
mod backup;
mod config;
mod files;
mod platforms;
mod plugin_market;
mod plugins;
mod providers;
mod status;

use astrbot_platform::PlatformManager;
use astrbot_plugin::PluginRegistry;
use astrbot_provider::ProviderManager;
use astrbot_runtime::RuntimeConfigService;
use axum::{
    Json, Router,
    extract::State,
    middleware,
    routing::{get, post},
};

pub use api_key::{
    ApiKeyAuthDecision, ApiKeyIssuer, ApiKeyRejectionReason, IssuedApiKey, OpenApiScope,
    OpenApiScopeSet, PresentedApiKey, authorize_api_key, extract_presented_api_key, hash_api_key,
};
pub use auth::{
    AuthRejectionReason, DashboardAuthDecision, DashboardAuthPolicy, ManagementAuthState,
    extract_bearer_token,
};
pub use backup::{
    ManagementBackupAbortRequest, ManagementBackupAbortResponse, ManagementBackupChunkRequest,
    ManagementBackupChunkResponse, ManagementBackupCompleteRequest,
    ManagementBackupCompleteResponse, ManagementBackupExportRequest, ManagementBackupImportRequest,
    ManagementBackupJobResponse, ManagementBackupPrecheckRequest, ManagementBackupPrecheckResponse,
    ManagementBackupProgressResponse, ManagementBackupState, ManagementBackupUploadStartRequest,
    ManagementBackupUploadStartResponse,
};
pub use config::{
    ManagementConfigMutationRequest, ManagementConfigMutationResponse,
    ManagementConfigSchemaResponse,
};
pub use files::{ManagementFileDownloadState, ScopedDownloadError, ScopedDownloadFile};
pub use platforms::PlatformManagementResponse;
pub use plugin_market::{
    PluginMarketCatalogResponse, PluginMarketManagementState, PluginMarketPlanRequest,
    PluginMarketPlanResponse,
};
pub use plugins::{PluginHandlerManagementResponse, PluginManagementResponse};
pub use providers::ProviderManagementResponse;
pub use status::ManagementStatusResponse;

#[derive(Clone, Debug)]
pub struct ManagementApiState {
    providers: ProviderManagementResponse,
    platforms: PlatformManagementResponse,
    plugins: PluginManagementResponse,
    config_service: Option<RuntimeConfigService>,
    plugin_market: Option<PluginMarketManagementState>,
    file_downloads: Option<ManagementFileDownloadState>,
    backup: Option<ManagementBackupState>,
}

impl ManagementApiState {
    pub fn new(
        providers: ProviderManagementResponse,
        platforms: PlatformManagementResponse,
        plugins: PluginManagementResponse,
    ) -> Self {
        Self {
            providers,
            platforms,
            plugins,
            config_service: None,
            plugin_market: None,
            file_downloads: None,
            backup: None,
        }
    }

    pub fn with_config_service(mut self, config_service: RuntimeConfigService) -> Self {
        self.config_service = Some(config_service);
        self
    }

    pub fn with_plugin_market(mut self, plugin_market: PluginMarketManagementState) -> Self {
        self.plugin_market = Some(plugin_market);
        self
    }

    pub fn with_file_downloads(mut self, file_downloads: ManagementFileDownloadState) -> Self {
        self.file_downloads = Some(file_downloads);
        self
    }

    pub fn with_backup(mut self, backup: ManagementBackupState) -> Self {
        self.backup = Some(backup);
        self
    }

    pub fn from_managers(
        provider_manager: &ProviderManager,
        platform_manager: &PlatformManager,
        plugin_registry: &PluginRegistry,
    ) -> Self {
        Self::new(
            ProviderManagementResponse::from_manager(provider_manager),
            PlatformManagementResponse::from_manager(platform_manager),
            PluginManagementResponse::from_registry(plugin_registry),
        )
    }

    pub fn providers(&self) -> &ProviderManagementResponse {
        &self.providers
    }

    pub fn platforms(&self) -> &PlatformManagementResponse {
        &self.platforms
    }

    pub fn plugins(&self) -> &PluginManagementResponse {
        &self.plugins
    }

    pub fn config_service(&self) -> Option<&RuntimeConfigService> {
        self.config_service.as_ref()
    }

    pub fn plugin_market(&self) -> Option<&PluginMarketManagementState> {
        self.plugin_market.as_ref()
    }

    pub fn file_downloads(&self) -> Option<&ManagementFileDownloadState> {
        self.file_downloads.as_ref()
    }

    pub fn backup(&self) -> Option<&ManagementBackupState> {
        self.backup.as_ref()
    }

    pub fn status(&self) -> ManagementStatusResponse {
        ManagementStatusResponse::new(
            self.providers.clone(),
            self.platforms.clone(),
            self.plugins.clone(),
        )
    }
}

pub fn management_router(state: ManagementApiState) -> Router {
    management_routes().with_state(state)
}

pub fn management_router_with_auth(state: ManagementApiState, auth: ManagementAuthState) -> Router {
    management_routes()
        .route_layer(middleware::from_fn_with_state(
            auth,
            auth::require_management_auth,
        ))
        .with_state(state)
}

fn management_routes() -> Router<ManagementApiState> {
    Router::new()
        .route("/api/management/status", get(status))
        .route("/api/management/providers", get(providers))
        .route("/api/management/platforms", get(platforms))
        .route("/api/management/plugins", get(plugins))
        .route("/api/management/config/schema", get(config::schema))
        .route(
            "/api/management/config/preview",
            post(config::preview_update),
        )
        .route("/api/management/config/apply", post(config::apply_update))
        .route("/api/management/plugin-market", get(plugin_market::catalog))
        .route("/api/management/files/{token}", get(files::download))
        .route("/api/management/backup/precheck", post(backup::precheck))
        .route("/api/management/backup/export", post(backup::export))
        .route("/api/management/backup/import", post(backup::import))
        .route(
            "/api/management/backup/progress/{task_id}",
            get(backup::progress),
        )
        .route(
            "/api/management/backup/upload/start",
            post(backup::upload_start),
        )
        .route(
            "/api/management/backup/upload/chunk",
            post(backup::upload_chunk),
        )
        .route(
            "/api/management/backup/upload/complete",
            post(backup::upload_complete),
        )
        .route(
            "/api/management/backup/upload/abort",
            post(backup::upload_abort),
        )
        .route(
            "/api/management/plugin-market/install-plan",
            post(plugin_market::install_plan),
        )
        .route(
            "/api/management/plugin-market/update-plan",
            post(plugin_market::update_plan),
        )
        .route(
            "/api/management/plugin-market/uninstall-plan",
            post(plugin_market::uninstall_plan),
        )
}

async fn status(State(state): State<ManagementApiState>) -> Json<ManagementStatusResponse> {
    Json(state.status())
}

async fn providers(State(state): State<ManagementApiState>) -> Json<ProviderManagementResponse> {
    Json(state.providers().clone())
}

async fn platforms(State(state): State<ManagementApiState>) -> Json<PlatformManagementResponse> {
    Json(state.platforms().clone())
}

async fn plugins(State(state): State<ManagementApiState>) -> Json<PluginManagementResponse> {
    Json(state.plugins().clone())
}
