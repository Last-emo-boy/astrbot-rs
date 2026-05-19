use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use astrbot_core::AstrbotError;
use astrbot_provider::{
    ANTHROPIC_CHAT_PROVIDER_TYPE, ChatRequest, GOOGLE_GENAI_CHAT_PROVIDER_TYPE,
    MOCK_CHAT_PROVIDER_TYPE, OPENAI_CHAT_PROVIDER_TYPE, ProviderCapability, ProviderManager,
    ProviderManagerConfigSet, ProviderModelDiscoveryConfig, ProviderModelDiscoverySupport,
    ProviderModelInfo, ProviderRegistry, default_model_candidates, discover_provider_models,
    model_discovery_support, model_metadata_map, provider_model_ids,
    sanitize_model_discovery_error,
};
use astrbot_runtime::{
    DEFAULT_ABCONF_ID, REDACTED_SECRET, RuntimeChatProviderConfig, RuntimeConfig,
    RuntimeConfigReloadAction, RuntimeConfigService, RuntimeProviderSourceConfig,
    validate_runtime_config_value,
};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderManagementResponse {
    pub chat_provider_count: usize,
    pub default_chat_provider_id: Option<String>,
    pub speech_to_text_provider_count: usize,
    pub default_speech_to_text_provider_id: Option<String>,
    pub text_to_speech_provider_count: usize,
    pub default_text_to_speech_provider_id: Option<String>,
    pub supports_text_to_speech_streaming: bool,
    pub embedding_provider_count: usize,
    pub default_embedding_provider_id: Option<String>,
    pub rerank_provider_count: usize,
    pub default_rerank_provider_id: Option<String>,
}

impl ProviderManagementResponse {
    pub fn from_manager(manager: &ProviderManager) -> Self {
        Self {
            chat_provider_count: manager.chat_provider_count(),
            default_chat_provider_id: manager.default_chat_provider_id().map(str::to_string),
            speech_to_text_provider_count: manager.speech_to_text_provider_count(),
            default_speech_to_text_provider_id: manager
                .default_speech_to_text_provider_id()
                .map(str::to_string),
            text_to_speech_provider_count: manager.text_to_speech_provider_count(),
            default_text_to_speech_provider_id: manager
                .default_text_to_speech_provider_id()
                .map(str::to_string),
            supports_text_to_speech_streaming: manager.supports_text_to_speech_streaming(),
            embedding_provider_count: manager.embedding_provider_count(),
            default_embedding_provider_id: manager
                .default_embedding_provider_id()
                .map(str::to_string),
            rerank_provider_count: manager.rerank_provider_count(),
            default_rerank_provider_id: manager.default_rerank_provider_id().map(str::to_string),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementProviderCatalogResponse {
    pub summary: ProviderManagementResponse,
    pub default_chat_provider_id: String,
    pub chat_providers: Vec<ManagementChatProviderDescriptor>,
    pub templates: Vec<ManagementProviderTemplate>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementChatProviderDescriptor {
    pub id: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub enabled: bool,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub timeout_secs: u64,
    pub mock_response: Option<String>,
    pub api_key_configured: bool,
    pub provider_source_id: Option<String>,
    pub provider: Option<String>,
    pub modalities: Vec<String>,
    pub max_context_tokens: Option<u64>,
}

impl From<RuntimeChatProviderConfig> for ManagementChatProviderDescriptor {
    fn from(config: RuntimeChatProviderConfig) -> Self {
        Self {
            id: config.id,
            provider_type: config.provider_type,
            enabled: config.enabled,
            model: config.model,
            api_base: config.api_base,
            timeout_secs: config.timeout_secs,
            mock_response: config.mock_response,
            api_key_configured: config.api_key.is_some_and(|api_key| !api_key.is_empty()),
            provider_source_id: config.provider_source_id,
            provider: config.provider,
            modalities: config.modalities,
            max_context_tokens: config.max_context_tokens,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementProviderTemplate {
    pub provider_type: String,
    pub label: String,
    pub default_model: Option<String>,
    pub default_api_base: Option<String>,
    pub requires_api_key: bool,
    pub capability: String,
    pub model_discovery: ProviderModelDiscoverySupport,
    pub model_candidates: Vec<ProviderModelInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementProviderUpsertRequest {
    pub provider: RuntimeChatProviderConfig,
    #[serde(default)]
    pub set_default: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementProviderDeleteRequest {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementProviderCheckRequest {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub provider: Option<RuntimeChatProviderConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementProviderModelsRequest {
    pub provider_type: String,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub provider: Option<RuntimeChatProviderConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyProviderListQuery {
    #[serde(default)]
    pub provider_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyProviderCheckQuery {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyProviderModelsQuery {
    #[serde(default)]
    pub provider_type: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementProviderMutationResponse {
    pub changed: bool,
    pub catalog: ManagementProviderCatalogResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementProviderCheckResponse {
    pub ok: bool,
    pub provider_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u128>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementProviderModelsResponse {
    pub provider_type: String,
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_candidates: Vec<ProviderModelInfo>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub model_metadata: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub dynamic: bool,
    #[serde(default)]
    pub unsupported: bool,
    pub capability: String,
    pub model_discovery: ProviderModelDiscoverySupport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl From<ManagementProviderModelsResult> for ManagementProviderModelsResponse {
    fn from(result: ManagementProviderModelsResult) -> Self {
        Self {
            provider_type: result.provider_type,
            models: result.models,
            model_candidates: result.model_candidates,
            model_metadata: result.model_metadata,
            source_id: result.source_id,
            source: result.source,
            dynamic: result.dynamic,
            unsupported: result.unsupported,
            capability: result.capability,
            model_discovery: result.model_discovery,
            error_kind: result.error_kind,
            message: result.message,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementProviderHealthResult {
    pub ok: bool,
    pub provider_id: String,
    pub status: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    pub elapsed_ms: u128,
}

impl ManagementProviderHealthResult {
    pub fn available(
        provider_id: impl Into<String>,
        message: impl Into<String>,
        elapsed_ms: u128,
    ) -> Self {
        Self {
            ok: true,
            provider_id: provider_id.into(),
            status: "available".to_string(),
            message: message.into(),
            error_kind: None,
            elapsed_ms,
        }
    }

    pub fn unavailable(
        provider_id: impl Into<String>,
        error_kind: impl Into<String>,
        message: impl Into<String>,
        elapsed_ms: u128,
    ) -> Self {
        Self {
            ok: false,
            provider_id: provider_id.into(),
            status: "unavailable".to_string(),
            message: message.into(),
            error_kind: Some(error_kind.into()),
            elapsed_ms,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementProviderModelsResult {
    pub provider_type: String,
    pub models: Vec<String>,
    pub model_candidates: Vec<ProviderModelInfo>,
    pub model_metadata: Map<String, Value>,
    pub dynamic: bool,
    pub unsupported: bool,
    pub capability: String,
    pub model_discovery: ProviderModelDiscoverySupport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub type ManagementProviderHealthFuture<'a> =
    Pin<Box<dyn Future<Output = astrbot_core::Result<ManagementProviderHealthResult>> + Send + 'a>>;

pub type ManagementProviderModelsFuture<'a> =
    Pin<Box<dyn Future<Output = astrbot_core::Result<ManagementProviderModelsResult>> + Send + 'a>>;

pub trait ManagementProviderHealthCheck: Send + Sync + std::fmt::Debug {
    fn check_provider<'a>(
        &'a self,
        provider: RuntimeChatProviderConfig,
    ) -> ManagementProviderHealthFuture<'a>;

    fn discover_models<'a>(
        &'a self,
        source: Option<RuntimeProviderSourceConfig>,
        provider_type: String,
    ) -> ManagementProviderModelsFuture<'a>;
}

#[derive(Clone, Debug, Default)]
pub struct DefaultManagementProviderHealthCheck;

impl ManagementProviderHealthCheck for DefaultManagementProviderHealthCheck {
    fn check_provider<'a>(
        &'a self,
        provider: RuntimeChatProviderConfig,
    ) -> ManagementProviderHealthFuture<'a> {
        Box::pin(async move { check_provider_with_runtime(provider).await })
    }

    fn discover_models<'a>(
        &'a self,
        source: Option<RuntimeProviderSourceConfig>,
        provider_type: String,
    ) -> ManagementProviderModelsFuture<'a> {
        Box::pin(async move { discover_provider_models_with_runtime(source, provider_type).await })
    }
}

pub async fn catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementProviderCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    catalog_response(&state, service)
        .map(Json)
        .map_err(map_provider_error)
}

pub async fn upsert(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementProviderUpsertRequest>,
) -> Result<Json<ManagementProviderMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    let mut config = service.read_config().map_err(map_provider_error)?;
    let mut provider = normalize_provider(request.provider).map_err(map_provider_error)?;
    if provider.api_key.as_deref() == Some(REDACTED_SECRET) {
        provider.api_key = config
            .chat_providers
            .iter()
            .find(|existing| existing.id == provider.id)
            .and_then(|existing| existing.api_key.clone());
    }

    let previous = config.chat_providers.clone();
    let previous_default = config.default_chat_provider_id.clone();
    if let Some(existing) = config
        .chat_providers
        .iter_mut()
        .find(|existing| existing.id == provider.id)
    {
        *existing = provider.clone();
    } else {
        config.chat_providers.push(provider.clone());
    }
    if request.set_default || config.default_chat_provider_id.trim().is_empty() {
        config.default_chat_provider_id = provider.id;
    }

    let changed =
        previous != config.chat_providers || previous_default != config.default_chat_provider_id;
    save_runtime_config(&state, service, config)
        .await
        .map_err(map_provider_error)?;
    Ok(Json(ManagementProviderMutationResponse {
        changed,
        catalog: catalog_response(&state, service).map_err(map_provider_error)?,
    }))
}

pub async fn delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementProviderDeleteRequest>,
) -> Result<Json<ManagementProviderMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    let mut config = service.read_config().map_err(map_provider_error)?;
    let id = non_empty_string(request.id)
        .ok_or_else(|| pipeline_error("provider id is required"))
        .map_err(map_provider_error)?;
    let before = config.chat_providers.len();
    config.chat_providers.retain(|provider| provider.id != id);
    let changed = before != config.chat_providers.len();
    if changed && config.default_chat_provider_id == id {
        config.default_chat_provider_id = config
            .chat_providers
            .first()
            .map(|provider| provider.id.clone())
            .unwrap_or_default();
    }
    save_runtime_config(&state, service, config)
        .await
        .map_err(map_provider_error)?;
    Ok(Json(ManagementProviderMutationResponse {
        changed,
        catalog: catalog_response(&state, service).map_err(map_provider_error)?,
    }))
}

pub async fn check(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementProviderCheckRequest>,
) -> Result<Json<ManagementProviderCheckResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    let provider = match request.provider {
        Some(provider) => normalize_provider(provider).map_err(map_provider_error)?,
        None => {
            let id = request
                .id
                .and_then(non_empty_string)
                .ok_or_else(|| pipeline_error("provider id is required"))
                .map_err(map_provider_error)?;
            service
                .read_config()
                .map_err(map_provider_error)?
                .chat_providers
                .into_iter()
                .find(|provider| provider.id == id)
                .ok_or_else(|| pipeline_error(format!("provider {id} is not configured")))
                .map_err(map_provider_error)?
        }
    };
    let result = state
        .provider_health_check()
        .check_provider(provider)
        .await
        .map_err(map_provider_error)?;

    Ok(Json(ManagementProviderCheckResponse {
        ok: result.ok,
        provider_id: result.provider_id,
        message: result.message,
        status: Some(result.status),
        error_kind: result.error_kind,
        elapsed_ms: Some(result.elapsed_ms),
    }))
}

pub async fn models(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementProviderModelsRequest>,
) -> Result<Json<ManagementProviderModelsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let provider_type = request.provider_type.trim().to_string();
    let source = match (request.source_id, request.provider) {
        (_, Some(provider)) => Some(provider_source_from_chat_provider(
            normalize_provider(provider).map_err(map_provider_error)?,
        )),
        (Some(source_id), None) => {
            let service = state
                .config_service()
                .ok_or_else(provider_config_unavailable)?;
            let source_id = non_empty_string(source_id)
                .ok_or_else(|| pipeline_error("source_id is required"))
                .map_err(map_provider_error)?;
            service
                .read_config()
                .map_err(map_provider_error)?
                .provider_sources
                .into_iter()
                .find(|source| source.id == source_id)
        }
        (None, None) => None,
    };
    let result = state
        .provider_health_check()
        .discover_models(source, provider_type)
        .await
        .map_err(map_provider_error)?;
    Ok(Json(ManagementProviderModelsResponse::from(result)))
}

pub async fn legacy_template(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    let config = service.read_config().map_err(map_provider_error)?;
    Ok(source_ok(json!({
        "config_schema": legacy_config_schema(),
        "provider_sources": legacy_provider_sources(&config),
        "providers": legacy_provider_values(&config, None),
    })))
}

pub async fn legacy_list(
    State(state): State<ManagementApiState>,
    Query(query): Query<LegacyProviderListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    let config = service.read_config().map_err(map_provider_error)?;
    Ok(source_ok(json!(legacy_provider_values(
        &config,
        query.provider_type.as_deref(),
    ))))
}

pub async fn legacy_create(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    legacy_upsert(state, None, payload).await
}

pub async fn legacy_update(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let original_id = payload
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let config = payload.get("config").cloned().unwrap_or(payload);
    legacy_upsert(state, original_id, config).await
}

pub async fn legacy_delete(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    let id = payload
        .get("id")
        .and_then(Value::as_str)
        .and_then(non_empty_string)
        .ok_or_else(|| pipeline_error("provider id is required"))
        .map_err(map_provider_error)?;
    let mut config = runtime_config_value(service).map_err(map_provider_error)?;
    let mut changed = false;
    for array_key in provider_array_keys() {
        if let Some(array) = config.get_mut(array_key).and_then(Value::as_array_mut) {
            let before = array.len();
            array.retain(|item| item.get("id").and_then(Value::as_str) != Some(id.as_str()));
            changed |= before != array.len();
        }
    }
    for default_key in provider_default_keys() {
        if config.get(default_key).and_then(Value::as_str) == Some(id.as_str()) {
            config[default_key] = if default_key == "default_chat_provider_id" {
                Value::String(String::new())
            } else {
                Value::Null
            };
            changed = true;
        }
    }
    normalize_default_provider_ids(&mut config);
    save_runtime_config_value(&state, service, config)
        .await
        .map_err(map_provider_error)?;
    Ok(source_ok(json!({ "changed": changed })))
}

pub async fn legacy_check_one(
    State(state): State<ManagementApiState>,
    Query(query): Query<LegacyProviderCheckQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    let config = service.read_config().map_err(map_provider_error)?;
    let id = query.id.trim();
    if id.is_empty() {
        return Err(map_provider_error(pipeline_error(
            "provider id is required",
        )));
    }
    let provider = config
        .chat_providers
        .into_iter()
        .find(|provider| provider.id == id)
        .ok_or_else(|| pipeline_error(format!("provider {id} is not configured")))
        .map_err(map_provider_error)?;
    let result = state
        .provider_health_check()
        .check_provider(provider)
        .await
        .map_err(map_provider_error)?;
    Ok(source_ok(json!({
        "id": result.provider_id,
        "name": result.provider_id,
        "status": result.status,
        "error": result.error_kind.as_ref().map(|_| result.message.clone()),
        "error_kind": result.error_kind,
        "message": result.message,
        "elapsed_ms": result.elapsed_ms,
    })))
}

pub async fn legacy_model_list(
    State(state): State<ManagementApiState>,
    Query(query): Query<LegacyProviderModelsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    let config = service.read_config().map_err(map_provider_error)?;
    let id = query
        .provider_id
        .as_deref()
        .or(query.source_id.as_deref())
        .and_then(non_empty_string)
        .ok_or_else(|| pipeline_error("provider_id or source_id is required"))
        .map_err(map_provider_error)?;
    let provider = config
        .chat_providers
        .iter()
        .find(|provider| provider.id == id)
        .cloned();
    let source = provider
        .as_ref()
        .and_then(|provider| provider.provider_source_id.as_deref())
        .and_then(|source_id| {
            config
                .provider_sources
                .iter()
                .find(|source| source.id == source_id)
                .cloned()
        });
    let provider_type = provider
        .as_ref()
        .map(|provider| provider.provider_type.clone())
        .or_else(|| source.as_ref().map(|source| source.provider_type.clone()))
        .or(query.provider_type)
        .unwrap_or_else(|| "chat_completion".to_string());
    let source = source.or_else(|| provider.map(provider_source_from_chat_provider));
    let result = state
        .provider_health_check()
        .discover_models(source, provider_type)
        .await
        .map_err(map_provider_error)?;
    Ok(source_ok(json!({
        "provider_type": result.provider_type,
        "provider_id": id,
        "source_id": result.source_id,
        "models": result.models,
        "model_metadata": legacy_model_metadata_for_models(&result.models),
        "model_candidates": result.model_candidates,
        "dynamic": result.dynamic,
        "unsupported": result.unsupported,
        "capability": result.capability,
        "model_discovery": result.model_discovery,
        "error_kind": result.error_kind,
        "message": result.message,
    })))
}

pub async fn legacy_source_models(
    State(state): State<ManagementApiState>,
    Query(query): Query<LegacyProviderModelsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    let config = service.read_config().map_err(map_provider_error)?;
    let source = query.source_id.as_deref().and_then(|source_id| {
        config
            .provider_sources
            .iter()
            .find(|source| source.id == source_id)
            .cloned()
    });
    let provider_type = source
        .as_ref()
        .map(|source| source.provider_type.clone())
        .or(query.provider_type)
        .unwrap_or_else(|| "chat_completion".to_string());
    let result = state
        .provider_health_check()
        .discover_models(source, provider_type)
        .await
        .map_err(map_provider_error)?;
    Ok(source_ok(json!({
        "models": result.models,
        "model_metadata": legacy_model_metadata_for_models(&result.models),
        "model_candidates": result.model_candidates,
        "source_id": result.source_id,
        "dynamic": result.dynamic,
        "unsupported": result.unsupported,
        "capability": result.capability,
        "model_discovery": result.model_discovery,
        "error_kind": result.error_kind,
        "message": result.message,
    })))
}

pub async fn legacy_source_update(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    let mut source = normalize_legacy_provider_source_payload(
        payload.get("config").cloned().unwrap_or(payload.clone()),
    )
    .map_err(map_provider_error)?;
    let source_id = source
        .get("id")
        .and_then(Value::as_str)
        .and_then(non_empty_string)
        .ok_or_else(|| pipeline_error("provider source id is required"))
        .map_err(map_provider_error)?;
    let original_id = payload
        .get("original_id")
        .and_then(Value::as_str)
        .and_then(non_empty_string)
        .unwrap_or_else(|| source_id.clone());
    let mut config = runtime_config_value(service).map_err(map_provider_error)?;
    let mut changed = false;
    preserve_redacted_source_secret(&config, &original_id, &mut source);
    let sources = config
        .get_mut("provider_sources")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| pipeline_error("provider_sources is unavailable"))
        .map_err(map_provider_error)?;
    if sources.iter().any(|existing| {
        existing.get("id").and_then(Value::as_str) == Some(source_id.as_str())
            && existing.get("id").and_then(Value::as_str) != Some(original_id.as_str())
    }) {
        return Err(map_provider_error(pipeline_error(format!(
            "provider source {source_id} already exists"
        ))));
    }
    if let Some(existing) = sources
        .iter_mut()
        .find(|existing| existing.get("id").and_then(Value::as_str) == Some(original_id.as_str()))
    {
        if existing != &source {
            *existing = source.clone();
            changed = true;
        }
    } else {
        sources.push(source.clone());
        changed = true;
    }
    if let Some(array) = config
        .get_mut("chat_providers")
        .and_then(Value::as_array_mut)
    {
        for provider in array.iter_mut().filter(|provider| {
            provider.get("provider_source_id").and_then(Value::as_str) == Some(original_id.as_str())
                || provider.get("id").and_then(Value::as_str) == Some(original_id.as_str())
        }) {
            let object = provider.as_object_mut().expect("provider object");
            object.insert(
                "provider_source_id".to_string(),
                Value::String(source_id.clone()),
            );
            copy_if_present(source.as_object(), object, "api_base");
            copy_if_present(source.as_object(), object, "api_key");
            copy_if_present(source.as_object(), object, "provider");
            copy_if_present(source.as_object(), object, "proxy");
            copy_if_present(source.as_object(), object, "timeout_secs");
            copy_if_present(source.as_object(), object, "custom_extra_body");
            if let Some(value) = source.get("type") {
                object.insert("type".to_string(), value.clone());
            }
            changed = true;
        }
    }
    save_runtime_config_value(&state, service, config)
        .await
        .map_err(map_provider_error)?;
    Ok(source_ok(json!({ "changed": changed })))
}

pub async fn legacy_source_delete(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    let id = payload
        .get("id")
        .and_then(Value::as_str)
        .and_then(non_empty_string)
        .ok_or_else(|| pipeline_error("provider source id is required"))
        .map_err(map_provider_error)?;
    let mut config = runtime_config_value(service).map_err(map_provider_error)?;
    let mut changed = false;
    if let Some(sources) = config
        .get_mut("provider_sources")
        .and_then(Value::as_array_mut)
    {
        let before = sources.len();
        sources.retain(|source| source.get("id").and_then(Value::as_str) != Some(id.as_str()));
        changed |= before != sources.len();
    }
    if let Some(array) = config
        .get_mut("chat_providers")
        .and_then(Value::as_array_mut)
    {
        let before = array.len();
        array.retain(|provider| {
            provider.get("provider_source_id").and_then(Value::as_str) != Some(id.as_str())
                && provider.get("id").and_then(Value::as_str) != Some(id.as_str())
        });
        changed |= before != array.len();
    }
    normalize_default_provider_ids(&mut config);
    save_runtime_config_value(&state, service, config)
        .await
        .map_err(map_provider_error)?;
    Ok(source_ok(json!({ "changed": changed })))
}

pub async fn legacy_embedding_dim(Json(payload): Json<Value>) -> Json<Value> {
    let dimension = payload
        .get("provider_config")
        .and_then(|provider| {
            provider
                .get("embedding_dimensions")
                .or_else(|| provider.get("dimensions"))
                .or_else(|| provider.get("dimension"))
        })
        .and_then(Value::as_u64)
        .or_else(|| {
            payload
                .get("provider_config")
                .and_then(|provider| provider.get("model"))
                .and_then(Value::as_str)
                .and_then(default_embedding_dimension)
        })
        .or_else(|| {
            payload
                .get("model")
                .and_then(Value::as_str)
                .and_then(default_embedding_dimension)
        })
        .or_else(|| {
            payload
                .get("embedding_dimensions")
                .or_else(|| payload.get("dimensions"))
                .or_else(|| payload.get("dimension"))
                .and_then(Value::as_u64)
        })
        .unwrap_or(1024);
    source_ok(json!({
        "embedding_dimensions": dimension,
        "dimension": dimension,
    }))
}

fn default_embedding_dimension(model: &str) -> Option<u64> {
    match model {
        "text-embedding-3-small" => Some(1536),
        "text-embedding-3-large" => Some(3072),
        "gemini-embedding-001" => Some(768),
        _ => None,
    }
}

fn catalog_response(
    state: &ManagementApiState,
    service: &RuntimeConfigService,
) -> astrbot_core::Result<ManagementProviderCatalogResponse> {
    let config = service.read_config()?;
    Ok(ManagementProviderCatalogResponse {
        summary: state.providers().clone(),
        default_chat_provider_id: config.default_chat_provider_id,
        chat_providers: config
            .chat_providers
            .into_iter()
            .map(ManagementChatProviderDescriptor::from)
            .collect(),
        templates: provider_templates(),
    })
}

async fn legacy_upsert(
    state: ManagementApiState,
    original_id: Option<String>,
    payload: Value,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(provider_config_unavailable)?;
    let mut provider = normalize_legacy_provider_payload(payload).map_err(map_provider_error)?;
    let id = provider
        .get("id")
        .and_then(Value::as_str)
        .and_then(non_empty_string)
        .ok_or_else(|| pipeline_error("provider id is required"))
        .map_err(map_provider_error)?;
    let category = legacy_provider_category(&provider);
    let array_key = provider_array_key(&category);
    let mut config = runtime_config_value(service).map_err(map_provider_error)?;
    if category == "chat_completion" {
        merge_source_into_chat_provider(&config, &mut provider);
    }
    let array = config
        .get_mut(array_key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| pipeline_error(format!("provider array {array_key} is unavailable")))
        .map_err(map_provider_error)?;
    let target_id = original_id.as_deref().unwrap_or(id.as_str());
    let mut changed = true;
    if let Some(existing) = array
        .iter_mut()
        .find(|existing| existing.get("id").and_then(Value::as_str) == Some(target_id))
    {
        if existing == &provider {
            changed = false;
        }
        *existing = provider.clone();
    } else {
        array.push(provider.clone());
    }
    set_default_provider_if_needed(&mut config, &category, &id);
    save_runtime_config_value(&state, service, config)
        .await
        .map_err(map_provider_error)?;
    provider = legacy_provider_output(provider, &category);
    Ok(source_ok(json!({
        "changed": changed,
        "provider": provider,
    })))
}

fn runtime_config_value(service: &RuntimeConfigService) -> astrbot_core::Result<Value> {
    serde_json::to_value(service.read_config()?)
        .map_err(|err| pipeline_error(format!("serialize runtime config: {err}")))
}

async fn save_runtime_config_value(
    state: &ManagementApiState,
    service: &RuntimeConfigService,
    value: Value,
) -> astrbot_core::Result<()> {
    let config = validate_runtime_config_value(value)?;
    save_runtime_config(state, service, config).await
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": "ok",
        "data": data,
    }))
}

fn legacy_provider_values(config: &RuntimeConfig, category: Option<&str>) -> Vec<Value> {
    let mut providers = Vec::new();
    push_legacy_chat_provider_values(
        &mut providers,
        serde_json::to_value(&config.chat_providers).unwrap_or_else(|_| json!([])),
        &legacy_provider_sources(config),
    );
    push_legacy_provider_values(
        &mut providers,
        "speech_to_text",
        serde_json::to_value(&config.speech_to_text_providers).unwrap_or_else(|_| json!([])),
    );
    push_legacy_provider_values(
        &mut providers,
        "text_to_speech",
        serde_json::to_value(&config.text_to_speech_providers).unwrap_or_else(|_| json!([])),
    );
    push_legacy_provider_values(
        &mut providers,
        "embedding",
        serde_json::to_value(&config.embedding_providers).unwrap_or_else(|_| json!([])),
    );
    push_legacy_provider_values(
        &mut providers,
        "rerank",
        serde_json::to_value(&config.rerank_providers).unwrap_or_else(|_| json!([])),
    );
    push_legacy_provider_values(
        &mut providers,
        "agent_runner",
        serde_json::to_value(&config.external_agent_runners).unwrap_or_else(|_| json!([])),
    );

    match category.and_then(non_empty_string) {
        Some(category) => providers
            .into_iter()
            .filter(|provider| {
                provider
                    .get("provider_type")
                    .and_then(Value::as_str)
                    .is_some_and(|provider_type| provider_type == category)
            })
            .collect(),
        None => providers,
    }
}

fn push_legacy_provider_values(providers: &mut Vec<Value>, category: &str, value: Value) {
    if let Some(array) = value.as_array() {
        providers.extend(
            array
                .iter()
                .cloned()
                .map(|provider| legacy_provider_output(provider, category)),
        );
    }
}

fn push_legacy_chat_provider_values(providers: &mut Vec<Value>, value: Value, sources: &[Value]) {
    if let Some(array) = value.as_array() {
        providers.extend(array.iter().cloned().map(|provider| {
            let mut provider = provider;
            merge_source_value_into_chat_provider(sources, &mut provider);
            legacy_provider_output(provider, "chat_completion")
        }));
    }
}

fn legacy_provider_output(mut provider: Value, category: &str) -> Value {
    let object = provider.as_object_mut().expect("provider object");
    object.insert(
        "provider_type".to_string(),
        Value::String(category.to_string()),
    );
    if let Some(enabled) = object.get("enabled").cloned() {
        object.insert("enable".to_string(), enabled);
    }
    if let Some(api_key) = object.get("api_key").and_then(Value::as_str) {
        if !api_key.is_empty() {
            object.insert(
                "key".to_string(),
                Value::String(REDACTED_SECRET.to_string()),
            );
            object.insert(
                "api_key".to_string(),
                Value::String(REDACTED_SECRET.to_string()),
            );
        }
    }
    if let Some(dimensions) = object.get("dimensions").cloned() {
        object.insert("embedding_dimensions".to_string(), dimensions);
    }
    if category == "chat_completion" {
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if !object.contains_key("provider_source_id") {
            object.insert("provider_source_id".to_string(), Value::String(id));
        }
        if !object.contains_key("provider") {
            let provider_kind = object
                .get("type")
                .and_then(Value::as_str)
                .map(legacy_provider_kind)
                .unwrap_or_else(|| "openai".to_string());
            object.insert("provider".to_string(), Value::String(provider_kind));
        }
    }
    provider
}

fn legacy_provider_sources(config: &RuntimeConfig) -> Vec<Value> {
    let mut sources: Vec<Value> = Vec::new();
    for source in serde_json::to_value(&config.provider_sources)
        .unwrap_or_else(|_| json!([]))
        .as_array()
        .into_iter()
        .flatten()
        .cloned()
    {
        push_legacy_provider_source(&mut sources, source);
    }
    for provider in serde_json::to_value(&config.chat_providers)
        .unwrap_or_else(|_| json!([]))
        .as_array()
        .into_iter()
        .flatten()
        .cloned()
        .map(|provider| legacy_provider_output(provider, "chat_completion"))
    {
        let object = provider.as_object().expect("provider object");
        let source_id = object
            .get("provider_source_id")
            .or_else(|| object.get("id"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if source_id.is_empty()
            || sources
                .iter()
                .any(|source| source.get("id").and_then(Value::as_str) == Some(source_id))
        {
            continue;
        }
        push_legacy_provider_source(
            &mut sources,
            json!({
                "id": source_id,
                "type": object.get("type").cloned().unwrap_or_else(|| Value::String(OPENAI_CHAT_PROVIDER_TYPE.to_string())),
                "provider_type": "chat_completion",
                "provider": object.get("provider").cloned().unwrap_or_else(|| Value::String("openai".to_string())),
                "enable": object.get("enable").cloned().unwrap_or(Value::Bool(true)),
                "api_base": object.get("api_base").cloned().unwrap_or(Value::Null),
                "api_key": object.get("api_key").cloned().unwrap_or(Value::Null),
                "proxy": object.get("proxy").cloned().unwrap_or(Value::Null),
            }),
        );
    }
    sources
}

fn push_legacy_provider_source(sources: &mut Vec<Value>, mut source: Value) {
    let Some(object) = source.as_object_mut() else {
        return;
    };
    let source_id = object
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if source_id.is_empty()
        || sources
            .iter()
            .any(|existing| existing.get("id").and_then(Value::as_str) == Some(source_id.as_str()))
    {
        return;
    }
    if let Some(enabled) = object.get("enabled").cloned() {
        object.insert("enable".to_string(), enabled);
    }
    object
        .entry("provider_type".to_string())
        .or_insert_with(|| Value::String("chat_completion".to_string()));
    if !object.contains_key("provider") {
        let provider_kind = object
            .get("type")
            .and_then(Value::as_str)
            .map(legacy_provider_kind)
            .unwrap_or_else(|| "openai".to_string());
        object.insert("provider".to_string(), Value::String(provider_kind));
    }
    if object
        .get("api_key")
        .and_then(Value::as_str)
        .is_some_and(|api_key| !api_key.is_empty())
    {
        object.insert(
            "key".to_string(),
            Value::String(REDACTED_SECRET.to_string()),
        );
        object.insert(
            "api_key".to_string(),
            Value::String(REDACTED_SECRET.to_string()),
        );
    } else {
        object.entry("key".to_string()).or_insert(Value::Null);
    }
    sources.push(source);
}

fn normalize_legacy_provider_payload(mut payload: Value) -> astrbot_core::Result<Value> {
    let category = legacy_provider_category(&payload);
    let object = payload
        .as_object_mut()
        .ok_or_else(|| pipeline_error("provider payload must be an object"))?;
    if let Some(value) = object.get("enable").cloned() {
        object.entry("enabled".to_string()).or_insert(value);
    }
    if let Some(value) = object.get("key").cloned() {
        object.entry("api_key".to_string()).or_insert(value);
    }
    if object.get("api_key").and_then(Value::as_str) == Some(REDACTED_SECRET) {
        object.remove("api_key");
    }
    if let Some(value) = object.get("embedding_dimensions").cloned() {
        object.entry("dimensions".to_string()).or_insert(value);
    }
    if category == "chat_completion" {
        if !object.contains_key("type") {
            object.insert(
                "type".to_string(),
                Value::String(OPENAI_CHAT_PROVIDER_TYPE.to_string()),
            );
        }
        if !object.contains_key("provider_source_id") {
            if let Some(id) = object.get("id").cloned() {
                object.insert("provider_source_id".to_string(), id);
            }
        }
        object.remove("provider_type");
    }
    if category != "chat_completion" {
        object.remove("provider_type");
    }
    Ok(payload)
}

fn normalize_legacy_provider_source_payload(mut payload: Value) -> astrbot_core::Result<Value> {
    let object = payload
        .as_object_mut()
        .ok_or_else(|| pipeline_error("provider source payload must be an object"))?;
    if let Some(value) = object.get("enable").cloned() {
        object.entry("enabled".to_string()).or_insert(value);
    }
    if let Some(value) = object.get("key").cloned() {
        object.entry("api_key".to_string()).or_insert(value);
    }
    object
        .entry("provider_type".to_string())
        .or_insert_with(|| Value::String("chat_completion".to_string()));
    if !object.contains_key("type") {
        object.insert(
            "type".to_string(),
            Value::String(OPENAI_CHAT_PROVIDER_TYPE.to_string()),
        );
    }
    if !object.contains_key("provider") {
        let provider_kind = object
            .get("type")
            .and_then(Value::as_str)
            .map(legacy_provider_kind)
            .unwrap_or_else(|| "openai".to_string());
        object.insert("provider".to_string(), Value::String(provider_kind));
    }
    object.remove("key");
    object.remove("enable");
    Ok(payload)
}

fn preserve_redacted_source_secret(config: &Value, original_id: &str, source: &mut Value) {
    let source_key_is_redacted =
        source.get("api_key").and_then(Value::as_str) == Some(REDACTED_SECRET);
    if !source_key_is_redacted {
        return;
    }
    let existing_key = config
        .get("provider_sources")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|existing| existing.get("id").and_then(Value::as_str) == Some(original_id))
        .and_then(|existing| existing.get("api_key").or_else(|| existing.get("key")))
        .cloned()
        .or_else(|| {
            config
                .get("chat_providers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|provider| {
                    provider.get("provider_source_id").and_then(Value::as_str) == Some(original_id)
                })
                .and_then(|provider| provider.get("api_key").or_else(|| provider.get("key")))
                .cloned()
        });
    if let Some(existing_key) = existing_key {
        source["api_key"] = existing_key;
    } else if let Some(object) = source.as_object_mut() {
        object.remove("api_key");
    }
}

fn merge_source_into_chat_provider(config: &Value, provider: &mut Value) {
    let Some(source_id) = provider
        .get("provider_source_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
    else {
        return;
    };
    let source = config
        .get("provider_sources")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|source| source.get("id").and_then(Value::as_str) == Some(source_id.as_str()))
        .cloned();
    if let Some(source) = source {
        merge_source_fields_into_provider(&source, provider);
    }
}

fn merge_source_value_into_chat_provider(sources: &[Value], provider: &mut Value) {
    let Some(source_id) = provider
        .get("provider_source_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
    else {
        return;
    };
    if let Some(source) = sources
        .iter()
        .find(|source| source.get("id").and_then(Value::as_str) == Some(source_id.as_str()))
    {
        merge_source_fields_into_provider(source, provider);
    }
}

fn merge_source_fields_into_provider(source: &Value, provider: &mut Value) {
    let object = provider.as_object_mut().expect("provider object");
    for key in [
        "type",
        "api_base",
        "api_key",
        "provider",
        "proxy",
        "timeout_secs",
        "custom_extra_body",
    ] {
        if let Some(value) = source.get(key).cloned() {
            if key == "api_key" && value.is_null() {
                continue;
            }
            object.insert(key.to_string(), value);
        }
    }
}

fn legacy_provider_category(provider: &Value) -> String {
    if let Some(category) = provider
        .get("provider_type")
        .and_then(Value::as_str)
        .filter(|category| {
            provider_array_key(category) != "chat_providers" || *category == "chat_completion"
        })
    {
        return category.to_string();
    }
    let provider_type = provider
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if provider_type.contains("speech_to_text") || provider_type.contains("stt") {
        "speech_to_text".to_string()
    } else if provider_type.contains("text_to_speech") || provider_type.contains("tts") {
        "text_to_speech".to_string()
    } else if provider_type.contains("embedding") {
        "embedding".to_string()
    } else if provider_type.contains("rerank") {
        "rerank".to_string()
    } else if matches!(
        provider_type,
        "dify" | "coze" | "dashscope" | "deerflow" | "fastgpt"
    ) {
        "agent_runner".to_string()
    } else {
        "chat_completion".to_string()
    }
}

fn provider_array_key(category: &str) -> &'static str {
    match category {
        "agent_runner" => "external_agent_runners",
        "speech_to_text" => "speech_to_text_providers",
        "text_to_speech" => "text_to_speech_providers",
        "embedding" => "embedding_providers",
        "rerank" => "rerank_providers",
        _ => "chat_providers",
    }
}

fn provider_array_keys() -> [&'static str; 6] {
    [
        "chat_providers",
        "external_agent_runners",
        "speech_to_text_providers",
        "text_to_speech_providers",
        "embedding_providers",
        "rerank_providers",
    ]
}

fn provider_default_keys() -> [&'static str; 5] {
    [
        "default_chat_provider_id",
        "default_speech_to_text_provider_id",
        "default_text_to_speech_provider_id",
        "default_embedding_provider_id",
        "default_rerank_provider_id",
    ]
}

fn set_default_provider_if_needed(config: &mut Value, category: &str, id: &str) {
    let key = match category {
        "speech_to_text" => "default_speech_to_text_provider_id",
        "text_to_speech" => "default_text_to_speech_provider_id",
        "embedding" => "default_embedding_provider_id",
        "rerank" => "default_rerank_provider_id",
        "agent_runner" => return,
        _ => "default_chat_provider_id",
    };
    let is_empty = config
        .get(key)
        .map(|value| value.is_null() || value.as_str().is_some_and(str::is_empty))
        .unwrap_or(true);
    if is_empty {
        config[key] = Value::String(id.to_string());
    }
}

fn normalize_default_provider_ids(config: &mut Value) {
    for (array_key, default_key, chat_default) in [
        ("chat_providers", "default_chat_provider_id", true),
        (
            "speech_to_text_providers",
            "default_speech_to_text_provider_id",
            false,
        ),
        (
            "text_to_speech_providers",
            "default_text_to_speech_provider_id",
            false,
        ),
        (
            "embedding_providers",
            "default_embedding_provider_id",
            false,
        ),
        ("rerank_providers", "default_rerank_provider_id", false),
    ] {
        let ids: Vec<String> = config
            .get(array_key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|provider| provider.get("id").and_then(Value::as_str))
            .map(ToOwned::to_owned)
            .collect();
        let current = config.get(default_key).and_then(Value::as_str);
        if current.is_some_and(|id| id.is_empty() || ids.iter().any(|existing| existing == id)) {
            continue;
        }
        config[default_key] = if let Some(first) = ids.first() {
            Value::String(first.clone())
        } else if chat_default {
            Value::String(String::new())
        } else {
            Value::Null
        };
    }
}

fn legacy_config_schema() -> Value {
    json!({
        "provider": {
            "items": {
                "id": { "type": "string", "hint": "提供商唯一 ID" },
                "key": { "type": "password", "hint": "API Key" },
                "api_base": { "type": "string", "hint": "API Base URL" },
                "proxy": { "type": "string", "hint": "HTTP/HTTPS proxy" },
                "model": { "type": "string", "hint": "模型 ID" },
                "enable": { "type": "bool", "hint": "是否启用" },
                "dimensions": { "type": "number", "hint": "Embedding dimension" }
            },
            "config_template": {
                "Mock": {
                    "id": "mock",
                    "type": MOCK_CHAT_PROVIDER_TYPE,
                    "provider_type": "chat_completion",
                    "provider": "mock",
                    "enable": true,
                    "model": "mock-response",
                    "mock_response": "ok",
                    "api_base": null,
                    "key": null
                },
                "OpenAI": {
                    "id": "openai",
                    "type": OPENAI_CHAT_PROVIDER_TYPE,
                    "provider_type": "chat_completion",
                    "provider": "openai",
                    "enable": true,
                    "model": "gpt-4.1-mini",
                    "api_base": "https://api.openai.com/v1",
                    "key": null
                },
                "Anthropic": {
                    "id": "anthropic",
                    "type": ANTHROPIC_CHAT_PROVIDER_TYPE,
                    "provider_type": "chat_completion",
                    "provider": "anthropic",
                    "enable": true,
                    "model": "claude-3-5-sonnet-latest",
                    "api_base": "https://api.anthropic.com",
                    "key": null
                },
                "Agent Runner": {
                    "id": "dify",
                    "type": "dify",
                    "provider_type": "agent_runner",
                    "provider": "dify",
                    "enable": true,
                    "api_base": "https://api.dify.ai/v1",
                    "key": null,
                    "app_id": null
                },
                "OpenAI STT": {
                    "id": "openai-stt",
                    "type": "openai_speech_to_text",
                    "provider_type": "speech_to_text",
                    "provider": "openai",
                    "enable": true,
                    "model": "whisper-1",
                    "api_base": "https://api.openai.com/v1",
                    "key": null
                },
                "OpenAI TTS": {
                    "id": "openai-tts",
                    "type": "openai_text_to_speech",
                    "provider_type": "text_to_speech",
                    "provider": "openai",
                    "enable": true,
                    "model": "tts-1",
                    "voice": "alloy",
                    "api_base": "https://api.openai.com/v1",
                    "key": null
                },
                "OpenAI Embedding": {
                    "id": "openai-embedding",
                    "type": "openai_embedding",
                    "provider_type": "embedding",
                    "provider": "openai",
                    "enable": true,
                    "model": "text-embedding-3-small",
                    "dimensions": 1024,
                    "api_base": "https://api.openai.com/v1",
                    "key": null
                },
                "vLLM Rerank": {
                    "id": "vllm-rerank",
                    "type": "vllm_rerank",
                    "provider_type": "rerank",
                    "provider": "vllm",
                    "enable": true,
                    "model": "BAAI/bge-reranker-v2-m3",
                    "api_base": "http://127.0.0.1:8000",
                    "key": null
                }
            }
        }
    })
}

fn legacy_model_metadata() -> Value {
    json!({
        "gpt-4.1-mini": {
            "modalities": { "input": ["text", "image"] },
            "tool_call": true,
            "reasoning": false,
            "limit": { "context": 1048576 }
        },
        "gpt-4.1": {
            "modalities": { "input": ["text", "image"] },
            "tool_call": true,
            "reasoning": false,
            "limit": { "context": 1048576 }
        },
        "o3-mini": {
            "modalities": { "input": ["text"] },
            "tool_call": true,
            "reasoning": true,
            "limit": { "context": 200000 }
        }
    })
}

fn legacy_model_metadata_for_models(models: &[String]) -> Value {
    let metadata = legacy_model_metadata();
    let Some(object) = metadata.as_object() else {
        return json!({});
    };
    let mut selected = Map::new();
    for model in models {
        if let Some(value) = object.get(model) {
            selected.insert(model.clone(), value.clone());
        }
    }
    Value::Object(selected)
}

fn legacy_provider_kind(provider_type: &str) -> String {
    provider_type
        .split('_')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("openai")
        .to_string()
}

async fn check_provider_with_runtime(
    provider: RuntimeChatProviderConfig,
) -> astrbot_core::Result<ManagementProviderHealthResult> {
    let provider_id = provider.id.clone();
    let timeout = Duration::from_secs(provider.timeout_secs.clamp(1, 120));
    let manager = ProviderManager::from_configs(
        &ProviderRegistry::with_builtin_chat_providers(),
        ProviderManagerConfigSet {
            chat_providers: vec![provider.clone().into()],
            default_chat_provider_id: Some(provider_id.clone()),
            ..ProviderManagerConfigSet::default()
        },
    )?;
    let Some(chat_provider) = manager.chat_provider(&provider_id) else {
        return Ok(ManagementProviderHealthResult::unavailable(
            provider_id,
            "configuration",
            "provider configuration did not create a runtime chat provider",
            0,
        ));
    };
    let started = Instant::now();
    let request = ChatRequest::new("ping", "management-health-check")
        .with_provider_id(provider_id.clone())
        .with_model(provider.model.unwrap_or_default());
    match tokio::time::timeout(timeout, chat_provider.chat(request)).await {
        Ok(Ok(_)) => {
            let _ = manager.terminate().await;
            Ok(ManagementProviderHealthResult::available(
                provider_id,
                "provider runtime responded to a lightweight chat request",
                started.elapsed().as_millis(),
            ))
        }
        Ok(Err(error)) => {
            let _ = manager.terminate().await;
            Ok(ManagementProviderHealthResult::unavailable(
                provider_id,
                classify_provider_error(&error),
                sanitize_provider_error(&error),
                started.elapsed().as_millis(),
            ))
        }
        Err(_) => {
            let _ = manager.terminate().await;
            Ok(ManagementProviderHealthResult::unavailable(
                provider_id,
                "timeout",
                format!(
                    "provider health check timed out after {}s",
                    timeout.as_secs()
                ),
                started.elapsed().as_millis(),
            ))
        }
    }
}

async fn discover_provider_models_with_runtime(
    source: Option<RuntimeProviderSourceConfig>,
    provider_type: String,
) -> astrbot_core::Result<ManagementProviderModelsResult> {
    let Some(source) = source else {
        let provider_type = provider_type.trim().to_string();
        let model_candidates = model_suggestions(&provider_type);
        let model_discovery = model_discovery_support(&provider_type);
        return Ok(ManagementProviderModelsResult {
            models: provider_model_ids(&model_candidates),
            model_metadata: web_model_metadata_map(&model_candidates),
            model_candidates,
            provider_type,
            dynamic: false,
            unsupported: model_discovery == ProviderModelDiscoverySupport::Unsupported,
            capability: ProviderCapability::ChatCompletion.to_string(),
            model_discovery,
            source_id: None,
            source: Some("static-suggestion".to_string()),
            error_kind: None,
            message: Some(
                "no provider source was supplied; returned built-in suggestions".to_string(),
            ),
        });
    };

    let provider_type = if provider_type.trim().is_empty() {
        source.provider_type.clone()
    } else {
        provider_type.trim().to_string()
    };
    let timeout = Duration::from_secs(source.timeout_secs.clamp(1, 120));
    let api_base = source
        .api_base
        .clone()
        .and_then(non_empty_string)
        .ok_or_else(|| {
            pipeline_error("provider source api_base is required for model discovery")
        })?;
    let mut discovery_config =
        ProviderModelDiscoveryConfig::new(provider_type.clone()).with_api_base(api_base);
    discovery_config.timeout = timeout;
    discovery_config.api_key = source.api_key.clone().and_then(non_empty_string);
    discovery_config.custom_headers = custom_headers_from_source(&source)?;

    let result = discover_provider_models(discovery_config).await;
    match result {
        Ok(result) => {
            let model_candidates = if result.models.is_empty() {
                model_suggestions(&provider_type)
            } else {
                result.models
            };
            Ok(ManagementProviderModelsResult {
                provider_type: result.provider_type,
                models: provider_model_ids(&model_candidates),
                model_metadata: web_model_metadata_map(&model_candidates),
                model_candidates,
                dynamic: result.dynamic,
                unsupported: result.unsupported,
                capability: ProviderCapability::ChatCompletion.to_string(),
                model_discovery: result.support,
                source_id: Some(source.id),
                source: Some(result.source),
                error_kind: result.error_kind,
                message: result.message,
            })
        }
        Err(error) => {
            let model_candidates = model_suggestions(&provider_type);
            Ok(ManagementProviderModelsResult {
                provider_type,
                models: provider_model_ids(&model_candidates),
                model_metadata: web_model_metadata_map(&model_candidates),
                model_candidates,
                dynamic: false,
                unsupported: false,
                capability: ProviderCapability::ChatCompletion.to_string(),
                model_discovery: model_discovery_support(source.provider_type.as_str()),
                source_id: Some(source.id),
                source: Some("runtime-model-list".to_string()),
                error_kind: Some(classify_provider_error(&error)),
                message: Some(sanitize_provider_error_with_secrets(
                    &error,
                    &[source.api_key.as_deref()],
                )),
            })
        }
    }
}

fn custom_headers_from_source(
    source: &RuntimeProviderSourceConfig,
) -> astrbot_core::Result<std::collections::HashMap<String, String>> {
    match source
        .custom_extra_body
        .get("custom_headers")
        .and_then(Value::as_object)
    {
        Some(headers) => Ok(headers
            .iter()
            .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_string())))
            .collect::<std::collections::HashMap<_, _>>()),
        None => Ok(Default::default()),
    }
}

fn web_model_metadata_map(models: &[ProviderModelInfo]) -> Map<String, Value> {
    let mut metadata = Map::new();
    for (model, fields) in model_metadata_map(models) {
        metadata.insert(model, json!(fields));
    }
    metadata
}

fn provider_source_from_chat_provider(
    provider: RuntimeChatProviderConfig,
) -> RuntimeProviderSourceConfig {
    RuntimeProviderSourceConfig {
        id: provider.id,
        provider_type: provider.provider_type,
        enabled: provider.enabled,
        provider: provider.provider,
        api_base: provider.api_base,
        api_key: provider.api_key,
        proxy: None,
        timeout_secs: provider.timeout_secs,
        custom_extra_body: provider.custom_extra_body,
    }
}

fn classify_provider_error(error: &AstrbotError) -> String {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("timed out") || message.contains("timeout") {
        "timeout".to_string()
    } else if message.contains("401")
        || message.contains("403")
        || message.contains("unauthorized")
        || message.contains("forbidden")
        || message.contains("api key")
        || message.contains("credential")
        || message.contains("auth")
    {
        "credential".to_string()
    } else if message.contains("dns")
        || message.contains("connect")
        || message.contains("connection")
        || message.contains("network")
    {
        "network".to_string()
    } else {
        "provider".to_string()
    }
}

fn sanitize_provider_error(error: &AstrbotError) -> String {
    sanitize_provider_error_with_secrets(error, &[])
}

fn sanitize_provider_error_with_secrets(error: &AstrbotError, secrets: &[Option<&str>]) -> String {
    let mut secret_values = secrets
        .iter()
        .filter_map(|secret| *secret)
        .collect::<Vec<_>>();
    secret_values.push(REDACTED_SECRET);
    sanitize_model_discovery_error(error, &secret_values)
}

fn copy_if_present(
    source: Option<&Map<String, Value>>,
    target: &mut Map<String, Value>,
    key: &str,
) {
    if let Some(value) = source.and_then(|source| source.get(key)).cloned() {
        target.insert(key.to_string(), value);
    }
}

fn provider_templates() -> Vec<ManagementProviderTemplate> {
    vec![
        ManagementProviderTemplate {
            provider_type: MOCK_CHAT_PROVIDER_TYPE.to_string(),
            label: "Mock".to_string(),
            default_model: None,
            default_api_base: None,
            requires_api_key: false,
            capability: ProviderCapability::ChatCompletion.to_string(),
            model_discovery: model_discovery_support(MOCK_CHAT_PROVIDER_TYPE),
            model_candidates: default_model_candidates(MOCK_CHAT_PROVIDER_TYPE),
        },
        ManagementProviderTemplate {
            provider_type: OPENAI_CHAT_PROVIDER_TYPE.to_string(),
            label: "OpenAI Compatible".to_string(),
            default_model: Some("chat-model".to_string()),
            default_api_base: Some("https://api.openai.com/v1".to_string()),
            requires_api_key: true,
            capability: ProviderCapability::ChatCompletion.to_string(),
            model_discovery: model_discovery_support(OPENAI_CHAT_PROVIDER_TYPE),
            model_candidates: default_model_candidates(OPENAI_CHAT_PROVIDER_TYPE),
        },
        ManagementProviderTemplate {
            provider_type: ANTHROPIC_CHAT_PROVIDER_TYPE.to_string(),
            label: "Anthropic".to_string(),
            default_model: Some("chat-model".to_string()),
            default_api_base: Some("https://api.anthropic.com".to_string()),
            requires_api_key: true,
            capability: ProviderCapability::ChatCompletion.to_string(),
            model_discovery: model_discovery_support(ANTHROPIC_CHAT_PROVIDER_TYPE),
            model_candidates: default_model_candidates(ANTHROPIC_CHAT_PROVIDER_TYPE),
        },
        ManagementProviderTemplate {
            provider_type: GOOGLE_GENAI_CHAT_PROVIDER_TYPE.to_string(),
            label: "Google GenAI".to_string(),
            default_model: Some("chat-model".to_string()),
            default_api_base: Some("https://generativelanguage.googleapis.com".to_string()),
            requires_api_key: true,
            capability: ProviderCapability::ChatCompletion.to_string(),
            model_discovery: model_discovery_support(GOOGLE_GENAI_CHAT_PROVIDER_TYPE),
            model_candidates: default_model_candidates(GOOGLE_GENAI_CHAT_PROVIDER_TYPE),
        },
    ]
}

fn model_suggestions(provider_type: &str) -> Vec<ProviderModelInfo> {
    default_model_candidates(provider_type)
}

fn normalize_provider(
    mut provider: RuntimeChatProviderConfig,
) -> astrbot_core::Result<RuntimeChatProviderConfig> {
    provider.id = non_empty_string(provider.id).ok_or_else(|| pipeline_error("id is required"))?;
    provider.provider_type = non_empty_string(provider.provider_type)
        .ok_or_else(|| pipeline_error("provider type is required"))?;
    provider.model = provider.model.and_then(non_empty_string);
    provider.api_base = provider.api_base.and_then(non_empty_string);
    provider.api_key = provider.api_key.and_then(non_empty_string);
    provider.mock_response = provider.mock_response.and_then(non_empty_string);
    Ok(provider)
}

async fn save_runtime_config(
    state: &ManagementApiState,
    service: &RuntimeConfigService,
    config: astrbot_runtime::RuntimeConfig,
) -> astrbot_core::Result<()> {
    let value = serde_json::to_value(config)
        .map_err(|err| pipeline_error(format!("serialize runtime config: {err}")))?;
    let preview = service.save_update_value(value)?;
    if preview.plan.reload_action != RuntimeConfigReloadAction::Noop {
        if let Some(executor) = state.config_apply() {
            executor
                .execute(
                    preview.config.clone(),
                    preview.plan.clone(),
                    DEFAULT_ABCONF_ID.to_string(),
                )
                .await?;
        }
    }
    Ok(())
}

fn provider_config_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "runtime config service is not configured".to_string(),
        }),
    )
}

fn map_provider_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn non_empty_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn pipeline_error(message: impl Into<String>) -> astrbot_core::AstrbotError {
    astrbot_core::AstrbotError::Pipeline(message.into())
}
