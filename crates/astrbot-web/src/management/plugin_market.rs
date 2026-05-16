use astrbot_plugin::{
    PluginInstallSource, PluginMarketEntry, PluginMarketOperationPlan, PluginPackageDescriptor,
    PluginUninstallPlan, PluginUpdatePlan,
};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone, Debug)]
pub struct PluginMarketManagementState {
    entries: Vec<PluginMarketEntry>,
}

impl PluginMarketManagementState {
    pub fn new(entries: Vec<PluginMarketEntry>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[PluginMarketEntry] {
        &self.entries
    }

    fn entry(&self, plugin_id: &str) -> Option<&PluginMarketEntry> {
        self.entries
            .iter()
            .find(|entry| entry.plugin_id == plugin_id)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginMarketCatalogResponse {
    pub plugins: Vec<PluginMarketEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginMarketPlanRequest {
    pub plugin_id: String,
    #[serde(default)]
    pub delete_config: bool,
    #[serde(default)]
    pub delete_data: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginMarketPlanResponse {
    pub plan: PluginMarketOperationPlan,
}

pub async fn catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<PluginMarketCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;

    Ok(Json(PluginMarketCatalogResponse {
        plugins: market.entries().to_vec(),
    }))
}

pub async fn install_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<PluginMarketPlanRequest>,
) -> Result<Json<PluginMarketPlanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    let entry = market.entry(&request.plugin_id).ok_or_else(|| {
        plugin_market_error(format!("plugin {} is not in market", request.plugin_id))
    })?;
    let plan = PluginMarketOperationPlan::from_market_entry(entry).ok_or_else(|| {
        plugin_market_error(format!(
            "plugin {} has no install source",
            request.plugin_id
        ))
    })?;

    Ok(Json(PluginMarketPlanResponse { plan }))
}

pub async fn update_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<PluginMarketPlanRequest>,
) -> Result<Json<PluginMarketPlanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let market = state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;
    let entry = market.entry(&request.plugin_id).ok_or_else(|| {
        plugin_market_error(format!("plugin {} is not in market", request.plugin_id))
    })?;
    let package = package_for_entry(entry).ok_or_else(|| {
        plugin_market_error(format!("plugin {} has no update source", request.plugin_id))
    })?;
    let plan = PluginMarketOperationPlan::update(
        PluginUpdatePlan::new(entry.plugin_id.clone(), package)
            .with_compatibility(entry.compatibility.clone()),
    );

    Ok(Json(PluginMarketPlanResponse { plan }))
}

pub async fn uninstall_plan(
    State(state): State<ManagementApiState>,
    Json(request): Json<PluginMarketPlanRequest>,
) -> Result<Json<PluginMarketPlanResponse>, (StatusCode, Json<ErrorResponse>)> {
    state
        .plugin_market()
        .ok_or_else(plugin_market_unavailable)?;

    let mut plan = PluginUninstallPlan::new(request.plugin_id);
    if request.delete_config {
        plan = plan.delete_config();
    }
    if request.delete_data {
        plan = plan.delete_data();
    }

    Ok(Json(PluginMarketPlanResponse {
        plan: PluginMarketOperationPlan::uninstall(plan),
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

fn plugin_market_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "plugin market state is not configured".to_string(),
        }),
    )
}

fn plugin_market_error(message: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse { error: message }),
    )
}
