mod api_key;
mod auth;
mod config;
mod platforms;
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
pub use config::{
    ManagementConfigMutationRequest, ManagementConfigMutationResponse,
    ManagementConfigSchemaResponse,
};
pub use platforms::PlatformManagementResponse;
pub use plugins::{PluginHandlerManagementResponse, PluginManagementResponse};
pub use providers::ProviderManagementResponse;
pub use status::ManagementStatusResponse;

#[derive(Clone, Debug)]
pub struct ManagementApiState {
    providers: ProviderManagementResponse,
    platforms: PlatformManagementResponse,
    plugins: PluginManagementResponse,
    config_service: Option<RuntimeConfigService>,
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
        }
    }

    pub fn with_config_service(mut self, config_service: RuntimeConfigService) -> Self {
        self.config_service = Some(config_service);
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
