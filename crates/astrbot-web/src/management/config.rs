use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use astrbot_runtime::{
    ConfigUiMetadata, DEFAULT_ABCONF_ID, RuntimeAbconfDescriptor, RuntimeAbconfRecord,
    RuntimeConfig, RuntimeConfigMutationPlan, RuntimeConfigReloadAction, RuntimeConfigSchema,
    RuntimeConfigService, RuntimeConfigUpdatePreview, UmopConfigRoute, UmopConfigRouter,
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
use super::platforms::{legacy_platform_config_value, legacy_platform_metadata_group};

#[derive(Clone)]
pub struct ManagementConfigRouteState {
    router: Arc<RwLock<UmopConfigRouter>>,
    store: Option<RuntimeConfigService>,
}

impl ManagementConfigRouteState {
    pub fn new(router: UmopConfigRouter) -> Self {
        Self {
            router: Arc::new(RwLock::new(router)),
            store: None,
        }
    }

    pub fn from_config_service(service: RuntimeConfigService) -> astrbot_core::Result<Self> {
        let router = service.read_umop_config_router()?;
        Ok(Self {
            router: Arc::new(RwLock::new(router)),
            store: Some(service),
        })
    }

    pub fn routes(&self) -> Result<Vec<UmopConfigRoute>, astrbot_core::AstrbotError> {
        self.router
            .read()
            .map_err(|error| astrbot_core::AstrbotError::Pipeline(error.to_string()))
            .map(|router| router.routes().to_vec())
    }

    pub fn replace_routes(
        &self,
        routes: Vec<UmopConfigRoute>,
    ) -> Result<Vec<UmopConfigRoute>, astrbot_core::AstrbotError> {
        let mut router = self
            .router
            .write()
            .map_err(|error| astrbot_core::AstrbotError::Pipeline(error.to_string()))?;
        router.replace_routes(routes)?;
        let routes = router.routes().to_vec();
        self.persist_routes(&routes)?;
        Ok(routes)
    }

    pub fn set_route(
        &self,
        pattern: String,
        config_id: String,
    ) -> Result<Vec<UmopConfigRoute>, astrbot_core::AstrbotError> {
        let mut router = self
            .router
            .write()
            .map_err(|error| astrbot_core::AstrbotError::Pipeline(error.to_string()))?;
        router.set_route(pattern, config_id)?;
        let routes = router.routes().to_vec();
        self.persist_routes(&routes)?;
        Ok(routes)
    }

    pub fn delete_route(
        &self,
        pattern: &str,
    ) -> Result<(bool, Vec<UmopConfigRoute>), astrbot_core::AstrbotError> {
        let mut router = self
            .router
            .write()
            .map_err(|error| astrbot_core::AstrbotError::Pipeline(error.to_string()))?;
        let deleted = router.delete_route(pattern)?;
        let routes = router.routes().to_vec();
        if deleted {
            self.persist_routes(&routes)?;
        }
        Ok((deleted, routes))
    }

    pub fn resolve_config_id(
        &self,
        umo: &str,
    ) -> Result<Option<String>, astrbot_core::AstrbotError> {
        self.router
            .read()
            .map_err(|error| astrbot_core::AstrbotError::Pipeline(error.to_string()))
            .map(|router| router.resolve_config_id(umo).map(str::to_string))
    }

    fn persist_routes(&self, routes: &[UmopConfigRoute]) -> astrbot_core::Result<()> {
        if let Some(store) = &self.store {
            store.save_umop_config_routes(routes)?;
        }
        Ok(())
    }
}

impl Default for ManagementConfigRouteState {
    fn default() -> Self {
        Self::new(UmopConfigRouter::default())
    }
}

impl std::fmt::Debug for ManagementConfigRouteState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementConfigRouteState")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ManagementConfigSchemaResponse {
    pub schema: RuntimeConfigSchema,
    pub ui_metadata: ConfigUiMetadata,
}

#[derive(Clone, Debug, Serialize)]
pub struct ManagementConfigCurrentResponse {
    pub config: RuntimeConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ManagementConfigMutationRequest {
    pub config: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conf_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ManagementConfigMutationResponse {
    pub config: RuntimeConfig,
    pub plan: RuntimeConfigMutationPlan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<ManagementConfigApplyExecution>,
}

impl ManagementConfigMutationResponse {
    fn from_preview(preview: RuntimeConfigUpdatePreview) -> Self {
        Self {
            config: preview.config,
            plan: preview.plan,
            execution: None,
        }
    }

    fn with_execution(mut self, execution: Option<ManagementConfigApplyExecution>) -> Self {
        self.execution = execution;
        self
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ManagementConfigApplyExecution {
    pub action: RuntimeConfigReloadAction,
    pub requested: bool,
    pub accepted: bool,
    pub message: String,
}

impl ManagementConfigApplyExecution {
    pub fn accepted(action: RuntimeConfigReloadAction, message: impl Into<String>) -> Self {
        Self {
            action,
            requested: true,
            accepted: true,
            message: message.into(),
        }
    }

    pub fn not_configured(action: RuntimeConfigReloadAction) -> Self {
        Self {
            action,
            requested: true,
            accepted: false,
            message: "runtime config apply executor is not configured".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ManagementConfigApplyExecutionRequest {
    pub config: RuntimeConfig,
    pub plan: RuntimeConfigMutationPlan,
    pub conf_id: String,
}

pub type ManagementConfigApplyFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ManagementConfigApplyExecution, String>> + Send + 'a>>;

pub trait ManagementConfigApplyExecutor: Send + Sync + std::fmt::Debug {
    fn apply_config_change<'a>(
        &'a self,
        request: ManagementConfigApplyExecutionRequest,
    ) -> ManagementConfigApplyFuture<'a>;
}

#[derive(Clone)]
pub struct ManagementConfigApplyState {
    executor: Arc<dyn ManagementConfigApplyExecutor>,
}

impl ManagementConfigApplyState {
    pub fn new(executor: Arc<dyn ManagementConfigApplyExecutor>) -> Self {
        Self { executor }
    }

    pub(crate) async fn execute(
        &self,
        config: RuntimeConfig,
        plan: RuntimeConfigMutationPlan,
        conf_id: String,
    ) -> Result<ManagementConfigApplyExecution, astrbot_core::AstrbotError> {
        self.executor
            .apply_config_change(ManagementConfigApplyExecutionRequest {
                config,
                plan,
                conf_id,
            })
            .await
            .map_err(astrbot_core::AstrbotError::Pipeline)
    }
}

#[derive(Clone)]
pub struct ManagementRuntimeConfigApplyController {
    handle: Arc<tokio::sync::Mutex<Option<astrbot_runtime::RuntimeHandle>>>,
}

impl ManagementRuntimeConfigApplyController {
    pub fn new(handle: astrbot_runtime::RuntimeHandle) -> Self {
        Self {
            handle: Arc::new(tokio::sync::Mutex::new(Some(handle))),
        }
    }

    pub fn apply_state(self) -> ManagementConfigApplyState {
        ManagementConfigApplyState::new(Arc::new(self))
    }

    pub async fn stop(&self) -> astrbot_core::Result<()> {
        let handle = self.handle.lock().await.take();
        if let Some(handle) = handle {
            handle.stop().await?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for ManagementRuntimeConfigApplyController {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementRuntimeConfigApplyController")
            .finish_non_exhaustive()
    }
}

impl ManagementConfigApplyExecutor for ManagementRuntimeConfigApplyController {
    fn apply_config_change<'a>(
        &'a self,
        request: ManagementConfigApplyExecutionRequest,
    ) -> ManagementConfigApplyFuture<'a> {
        Box::pin(async move {
            let action = request.plan.reload_action;
            let mut handle_slot = self.handle.lock().await;
            let Some(handle) = handle_slot.take() else {
                return Err("runtime handle is not available".to_string());
            };

            let handle = match action {
                RuntimeConfigReloadAction::Noop => handle,
                RuntimeConfigReloadAction::ReloadRuntime => handle
                    .reload(request.config)
                    .await
                    .map_err(|error| error.to_string())?,
                RuntimeConfigReloadAction::RestartRuntime => handle
                    .restart(request.config)
                    .await
                    .map_err(|error| error.to_string())?,
            };
            *handle_slot = Some(handle);

            Ok(ManagementConfigApplyExecution::accepted(
                action,
                format!("runtime {action:?} applied for {}", request.conf_id),
            ))
        })
    }
}

impl std::fmt::Debug for ManagementConfigApplyState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementConfigApplyState")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConfigRouteCatalogResponse {
    pub routes: Vec<UmopConfigRoute>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConfigRouteMutationResponse {
    pub changed: bool,
    pub routes: Vec<UmopConfigRoute>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConfigRouteUpsertRequest {
    pub pattern: String,
    pub config_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConfigRouteDeleteRequest {
    pub pattern: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConfigRouteReplaceRequest {
    pub routes: Vec<UmopConfigRoute>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConfigRouteResolveRequest {
    pub umo: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConfigRouteResolveResponse {
    pub umo: String,
    pub config_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyAbconfGetQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_config: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyUmoRouteReplaceRequest {
    pub routing: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyUmoRouteUpsertRequest {
    pub umo: String,
    pub conf_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyUmoRouteDeleteRequest {
    pub umo: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementAbconfCatalogResponse {
    pub info_list: Vec<RuntimeAbconfDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementAbconfResponse {
    pub abconf: RuntimeAbconfRecord,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementAbconfCreateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementAbconfCreateResponse {
    pub conf_id: String,
    pub abconf: RuntimeAbconfRecord,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementAbconfIdRequest {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementAbconfUpdateRequest {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementAbconfDeleteResponse {
    pub deleted: bool,
}

pub async fn schema(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementConfigSchemaResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;

    Ok(Json(ManagementConfigSchemaResponse {
        schema: service.schema(),
        ui_metadata: RuntimeConfig::ui_metadata(),
    }))
}

pub async fn current(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementConfigCurrentResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;

    let config = service.read_config().map_err(map_config_error)?;
    Ok(Json(ManagementConfigCurrentResponse { config }))
}

pub async fn preview_update(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConfigMutationRequest>,
) -> Result<Json<ManagementConfigMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let conf_id = normalized_conf_id(request.conf_id);
    let preview = service
        .preview_update_value_for_conf(&conf_id, request.config)
        .map_err(map_config_error)?;

    Ok(Json(ManagementConfigMutationResponse::from_preview(
        preview,
    )))
}

pub async fn apply_update(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConfigMutationRequest>,
) -> Result<Json<ManagementConfigMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let conf_id = normalized_conf_id(request.conf_id);
    let preview = service
        .save_update_value_for_conf(&conf_id, request.config)
        .map_err(map_config_error)?;
    let execution = match (conf_id.as_str(), preview.plan.reload_action) {
        (DEFAULT_ABCONF_ID, RuntimeConfigReloadAction::Noop) => None,
        (DEFAULT_ABCONF_ID, action) => Some(match state.config_apply() {
            Some(executor) => executor
                .execute(
                    preview.config.clone(),
                    preview.plan.clone(),
                    conf_id.clone(),
                )
                .await
                .map_err(map_config_error)?,
            None => ManagementConfigApplyExecution::not_configured(action),
        }),
        (_, RuntimeConfigReloadAction::Noop) => None,
        (_, action) => Some(ManagementConfigApplyExecution {
            action,
            requested: true,
            accepted: true,
            message: format!(
                "runtime config {conf_id} was persisted; routed scheduler reload is pending"
            ),
        }),
    };

    Ok(Json(
        ManagementConfigMutationResponse::from_preview(preview).with_execution(execution),
    ))
}

pub async fn abconf_catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementAbconfCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let info_list = service.list_abconfs().map_err(map_config_error)?;
    Ok(Json(ManagementAbconfCatalogResponse { info_list }))
}

pub async fn abconf_create(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementAbconfCreateRequest>,
) -> Result<Json<ManagementAbconfCreateResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let abconf = service
        .create_abconf(request.name, request.config)
        .map_err(map_config_error)?;
    Ok(Json(ManagementAbconfCreateResponse {
        conf_id: abconf.id.clone(),
        abconf,
    }))
}

pub async fn abconf_get(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementAbconfIdRequest>,
) -> Result<Json<ManagementAbconfResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let abconf = service
        .get_abconf(&request.id)
        .map_err(map_config_error)?
        .ok_or_else(|| map_not_found("abconf not found"))?;
    Ok(Json(ManagementAbconfResponse { abconf }))
}

pub async fn abconf_update(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementAbconfUpdateRequest>,
) -> Result<Json<ManagementAbconfResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let abconf = service
        .update_abconf_info(&request.id, request.name)
        .map_err(map_config_error)?
        .ok_or_else(|| map_not_found("abconf not found"))?;
    Ok(Json(ManagementAbconfResponse { abconf }))
}

pub async fn abconf_delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementAbconfIdRequest>,
) -> Result<Json<ManagementAbconfDeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let deleted = service
        .delete_abconf(&request.id)
        .map_err(map_config_error)?;
    Ok(Json(ManagementAbconfDeleteResponse { deleted }))
}

pub async fn route_catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementConfigRouteCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let routes = state
        .config_routes()
        .ok_or_else(config_routes_unavailable)?
        .routes()
        .map_err(map_config_error)?;

    Ok(Json(ManagementConfigRouteCatalogResponse { routes }))
}

pub async fn route_upsert(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConfigRouteUpsertRequest>,
) -> Result<Json<ManagementConfigRouteMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let routes = state
        .config_routes()
        .ok_or_else(config_routes_unavailable)?
        .set_route(request.pattern, request.config_id)
        .map_err(map_config_error)?;

    Ok(Json(ManagementConfigRouteMutationResponse {
        changed: true,
        routes,
    }))
}

pub async fn route_delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConfigRouteDeleteRequest>,
) -> Result<Json<ManagementConfigRouteMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let (deleted, routes) = state
        .config_routes()
        .ok_or_else(config_routes_unavailable)?
        .delete_route(&request.pattern)
        .map_err(map_config_error)?;

    Ok(Json(ManagementConfigRouteMutationResponse {
        changed: deleted,
        routes,
    }))
}

pub async fn route_replace(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConfigRouteReplaceRequest>,
) -> Result<Json<ManagementConfigRouteMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let routes = state
        .config_routes()
        .ok_or_else(config_routes_unavailable)?
        .replace_routes(request.routes)
        .map_err(map_config_error)?;

    Ok(Json(ManagementConfigRouteMutationResponse {
        changed: true,
        routes,
    }))
}

pub async fn route_resolve(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConfigRouteResolveRequest>,
) -> Result<Json<ManagementConfigRouteResolveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let config_id = state
        .config_routes()
        .ok_or_else(config_routes_unavailable)?
        .resolve_config_id(&request.umo)
        .map_err(map_config_error)?;

    Ok(Json(ManagementConfigRouteResolveResponse {
        umo: request.umo,
        config_id,
    }))
}

pub async fn legacy_abconf_catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let info_list = service.list_abconfs().map_err(map_config_error)?;
    Ok(legacy_ok(
        json!({ "info_list": info_list }),
        "获取配置列表成功",
    ))
}

pub async fn legacy_abconf_create(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementAbconfCreateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let abconf = service
        .create_abconf(request.name, request.config)
        .map_err(map_config_error)?;
    Ok(legacy_ok(
        json!({ "conf_id": abconf.id, "abconf": abconf }),
        "创建成功",
    ))
}

pub async fn legacy_abconf_get(
    State(state): State<ManagementApiState>,
    Query(query): Query<LegacyAbconfGetQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let system_config = query
        .system_config
        .as_deref()
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"));
    let conf_id = if system_config {
        DEFAULT_ABCONF_ID.to_string()
    } else {
        normalized_conf_id(query.id)
    };
    let config = service
        .read_config_for_conf(&conf_id)
        .map_err(map_config_error)?;
    Ok(legacy_ok(
        json!({
            "config": legacy_platform_config_value(config),
            "metadata": legacy_config_metadata(),
            "schema": service.schema(),
        }),
        "获取配置成功",
    ))
}

pub async fn legacy_default_config(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    Ok(legacy_ok(
        json!({
            "config": legacy_platform_config_value(RuntimeConfig::default()),
            "metadata": legacy_config_metadata(),
            "schema": service.schema(),
        }),
        "获取默认配置成功",
    ))
}

pub async fn legacy_current_config(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let config = service.read_config().map_err(map_config_error)?;
    Ok(legacy_ok(
        json!({
            "config": legacy_platform_config_value(config),
            "metadata": legacy_config_metadata(),
            "schema": service.schema(),
        }),
        "获取配置成功",
    ))
}

pub async fn legacy_abconf_update(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementAbconfUpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let abconf = service
        .update_abconf_info(&request.id, request.name)
        .map_err(map_config_error)?
        .ok_or_else(|| map_not_found("abconf not found"))?;
    Ok(legacy_ok(json!({ "abconf": abconf }), "更新成功"))
}

pub async fn legacy_abconf_delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementAbconfIdRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let deleted = service
        .delete_abconf(&request.id)
        .map_err(map_config_error)?;
    Ok(legacy_ok(json!({ "deleted": deleted }), "删除成功"))
}

pub async fn legacy_apply_update(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConfigMutationRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(config_service_unavailable)?;
    let conf_id = normalized_conf_id(request.conf_id);
    let preview = service
        .save_update_value_for_conf(&conf_id, request.config)
        .map_err(map_config_error)?;
    let execution = legacy_execute_apply(&state, &conf_id, &preview).await?;
    Ok(legacy_ok(
        json!({
            "config": preview.config,
            "plan": preview.plan,
            "execution": execution,
        }),
        "更新成功",
    ))
}

pub async fn legacy_route_catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let routes = state
        .config_routes()
        .ok_or_else(config_routes_unavailable)?
        .routes()
        .map_err(map_config_error)?;
    Ok(legacy_ok(
        json!({
            "routing": routes_to_legacy_routing(&routes),
            "routes": routes,
        }),
        "获取路由表成功",
    ))
}

pub async fn legacy_route_replace(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyUmoRouteReplaceRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let routes = request
        .routing
        .into_iter()
        .map(|(pattern, config_id)| UmopConfigRoute::new(pattern, config_id))
        .collect::<Vec<_>>();
    let routes = state
        .config_routes()
        .ok_or_else(config_routes_unavailable)?
        .replace_routes(routes)
        .map_err(map_config_error)?;
    Ok(legacy_ok(
        json!({
            "routing": routes_to_legacy_routing(&routes),
            "routes": routes,
        }),
        "更新成功",
    ))
}

pub async fn legacy_route_upsert(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyUmoRouteUpsertRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let routes = state
        .config_routes()
        .ok_or_else(config_routes_unavailable)?
        .set_route(request.umo, request.conf_id)
        .map_err(map_config_error)?;
    Ok(legacy_ok(
        json!({
            "routing": routes_to_legacy_routing(&routes),
            "routes": routes,
        }),
        "更新成功",
    ))
}

pub async fn legacy_route_delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyUmoRouteDeleteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let (_deleted, routes) = state
        .config_routes()
        .ok_or_else(config_routes_unavailable)?
        .delete_route(&request.umo)
        .map_err(map_config_error)?;
    Ok(legacy_ok(
        json!({
            "routing": routes_to_legacy_routing(&routes),
            "routes": routes,
        }),
        "删除成功",
    ))
}

async fn legacy_execute_apply(
    state: &ManagementApiState,
    conf_id: &str,
    preview: &RuntimeConfigUpdatePreview,
) -> Result<Option<ManagementConfigApplyExecution>, (StatusCode, Json<ErrorResponse>)> {
    let execution = match (conf_id, preview.plan.reload_action) {
        (DEFAULT_ABCONF_ID, RuntimeConfigReloadAction::Noop) => None,
        (DEFAULT_ABCONF_ID, action) => Some(match state.config_apply() {
            Some(executor) => executor
                .execute(
                    preview.config.clone(),
                    preview.plan.clone(),
                    conf_id.to_string(),
                )
                .await
                .map_err(map_config_error)?,
            None => ManagementConfigApplyExecution::not_configured(action),
        }),
        (_, RuntimeConfigReloadAction::Noop) => None,
        (_, action) => Some(ManagementConfigApplyExecution {
            action,
            requested: true,
            accepted: true,
            message: format!(
                "runtime config {conf_id} was persisted; routed scheduler reload is pending"
            ),
        }),
    };
    Ok(execution)
}

fn legacy_config_metadata() -> Value {
    let schema = RuntimeConfig::schema();
    let mut sections = serde_json::Map::new();
    for group in RuntimeConfig::ui_metadata().groups {
        let mut items = serde_json::Map::new();
        for field in group.fields {
            let field_schema = schema
                .fields
                .iter()
                .find(|candidate| candidate.path == field.path);
            items.insert(
                field.path.to_string(),
                json!({
                    "type": field.control,
                    "description": field.path,
                    "hint": "",
                    "secret": field.secret,
                    "default": field_schema.map(|schema| schema.default_value.clone()).unwrap_or(Value::Null),
                    "value_type": field_schema.map(|schema| json!(schema.value_type)).unwrap_or(Value::Null),
                }),
            );
        }
        sections.insert(
            group.id.to_string(),
            json!({
                "name": group.title,
                "metadata": {
                    group.id: {
                        "type": "object",
                        "description": group.title,
                        "items": items,
                    }
                }
            }),
        );
    }
    sections.insert(
        "platform_group".to_string(),
        legacy_platform_metadata_group(),
    );
    Value::Object(sections)
}

fn routes_to_legacy_routing(routes: &[UmopConfigRoute]) -> BTreeMap<String, String> {
    routes
        .iter()
        .map(|route| (route.pattern.clone(), route.config_id.clone()))
        .collect()
}

fn legacy_ok(data: Value, message: impl Into<String>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": message.into(),
        "data": data,
    }))
}

fn config_service_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "management config service is not configured".to_string(),
        }),
    )
}

fn config_routes_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "management config route state is not configured".to_string(),
        }),
    )
}

fn map_config_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn map_not_found(message: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}

fn normalized_conf_id(conf_id: Option<String>) -> String {
    conf_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_ABCONF_ID.to_string())
}
