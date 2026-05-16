mod platforms;
mod plugins;
mod providers;
mod status;

use astrbot_platform::PlatformManager;
use astrbot_plugin::PluginRegistry;
use astrbot_provider::ProviderManager;
use axum::{Json, Router, extract::State, routing::get};

pub use platforms::PlatformManagementResponse;
pub use plugins::{PluginHandlerManagementResponse, PluginManagementResponse};
pub use providers::ProviderManagementResponse;
pub use status::ManagementStatusResponse;

#[derive(Clone, Debug)]
pub struct ManagementApiState {
    providers: ProviderManagementResponse,
    platforms: PlatformManagementResponse,
    plugins: PluginManagementResponse,
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
        }
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

    pub fn status(&self) -> ManagementStatusResponse {
        ManagementStatusResponse::new(
            self.providers.clone(),
            self.platforms.clone(),
            self.plugins.clone(),
        )
    }
}

pub fn management_router(state: ManagementApiState) -> Router {
    Router::new()
        .route("/api/management/status", get(status))
        .route("/api/management/providers", get(providers))
        .route("/api/management/platforms", get(platforms))
        .route("/api/management/plugins", get(plugins))
        .with_state(state)
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
