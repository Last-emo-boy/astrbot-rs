use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_core::AstrbotError;
use astrbot_platform::{
    AIOCQHTTP_PLATFORM_TYPE, CONSOLE_PLATFORM_TYPE, DINGTALK_PLATFORM_TYPE, LARK_PLATFORM_TYPE,
    LINE_PLATFORM_TYPE, MOCK_PLATFORM_TYPE, ONEBOT_PLATFORM_TYPE, PlatformBuildContext,
    PlatformManager, PlatformRegistry, SLACK_PLATFORM_TYPE, TELEGRAM_PLATFORM_TYPE,
    WEBCHAT_PLATFORM_TYPE, WECOM_AI_BOT_PLATFORM_TYPE, WECOM_PLATFORM_TYPE,
};
use astrbot_runtime::{
    DEFAULT_ABCONF_ID, REDACTED_SECRET, RuntimeConfig, RuntimeConfigReloadAction,
    RuntimeConfigService, RuntimePlatformConfig,
};
use astrbot_storage::PlatformStatsRecord;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::sync::mpsc;

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformManagementResponse {
    pub platform_count: usize,
    pub platform_ids: Vec<String>,
    pub mock_platform_count: usize,
    pub webchat_platform_count: usize,
    pub onebot_platform_count: usize,
    pub recording_sink_count: usize,
}

impl PlatformManagementResponse {
    pub fn from_manager(manager: &PlatformManager) -> Self {
        Self {
            platform_count: manager.platform_count(),
            platform_ids: manager.platform_ids(),
            mock_platform_count: manager.mock_platform_count(),
            webchat_platform_count: manager.webchat_platform_count(),
            onebot_platform_count: manager.onebot_platform_count(),
            recording_sink_count: manager.recording_sink_count(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPlatformCatalogResponse {
    pub summary: PlatformManagementResponse,
    pub platforms: Vec<ManagementPlatformDescriptor>,
    pub templates: Vec<ManagementPlatformTemplate>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPlatformDescriptor {
    pub id: String,
    #[serde(rename = "type")]
    pub platform_type: String,
    pub enable: bool,
    pub enabled: bool,
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub secrets: BTreeMap<String, String>,
}

impl From<RuntimePlatformConfig> for ManagementPlatformDescriptor {
    fn from(config: RuntimePlatformConfig) -> Self {
        let secrets = redact_platform_secrets(&config.secrets);
        Self {
            id: config.id,
            platform_type: config.platform_type,
            enable: config.enabled,
            enabled: config.enabled,
            name: config.name,
            options: config.options,
            secrets,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPlatformTemplate {
    pub platform_type: String,
    pub label: String,
    pub runtime_supported: bool,
    pub config: RuntimePlatformConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPlatformUpsertRequest {
    pub platform: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPlatformDeleteRequest {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPlatformCheckRequest {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub platform: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPlatformMutationResponse {
    pub changed: bool,
    pub catalog: ManagementPlatformCatalogResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPlatformCheckResponse {
    pub ok: bool,
    pub platform_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u128>,
    #[serde(default)]
    pub webhook_reachable: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPlatformHealthResult {
    pub ok: bool,
    pub platform_id: String,
    pub status: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    pub elapsed_ms: u128,
    pub webhook_reachable: bool,
}

impl ManagementPlatformHealthResult {
    pub fn available(
        platform_id: impl Into<String>,
        message: impl Into<String>,
        elapsed_ms: u128,
        webhook_reachable: bool,
    ) -> Self {
        Self {
            ok: true,
            platform_id: platform_id.into(),
            status: "available".to_string(),
            message: message.into(),
            error_kind: None,
            elapsed_ms,
            webhook_reachable,
        }
    }

    pub fn unavailable(
        platform_id: impl Into<String>,
        error_kind: impl Into<String>,
        message: impl Into<String>,
        elapsed_ms: u128,
        webhook_reachable: bool,
    ) -> Self {
        Self {
            ok: false,
            platform_id: platform_id.into(),
            status: "unavailable".to_string(),
            message: message.into(),
            error_kind: Some(error_kind.into()),
            elapsed_ms,
            webhook_reachable,
        }
    }
}

pub type ManagementPlatformHealthFuture<'a> =
    Pin<Box<dyn Future<Output = astrbot_core::Result<ManagementPlatformHealthResult>> + Send + 'a>>;

pub trait ManagementPlatformHealthCheck: Send + Sync + std::fmt::Debug {
    fn check_platform<'a>(
        &'a self,
        platform: RuntimePlatformConfig,
    ) -> ManagementPlatformHealthFuture<'a>;
}

#[derive(Clone, Debug, Default)]
pub struct DefaultManagementPlatformHealthCheck;

impl ManagementPlatformHealthCheck for DefaultManagementPlatformHealthCheck {
    fn check_platform<'a>(
        &'a self,
        platform: RuntimePlatformConfig,
    ) -> ManagementPlatformHealthFuture<'a> {
        Box::pin(async move { check_platform_with_runtime(platform).await })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyPlatformUpdateRequest {
    pub id: String,
    pub config: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyPlatformDeleteRequest {
    pub id: String,
}

pub async fn catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementPlatformCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(platform_config_unavailable)?;
    catalog_response(&state, service)
        .map(Json)
        .map_err(map_platform_error)
}

pub async fn upsert(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPlatformUpsertRequest>,
) -> Result<Json<ManagementPlatformMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(platform_config_unavailable)?;
    let mut config = service.read_config().map_err(map_platform_error)?;
    let mut platform = normalize_platform_value(request.platform).map_err(map_platform_error)?;
    preserve_redacted_platform_secrets(&config.platforms, &mut platform);
    let previous = config.platforms.clone();
    if let Some(existing) = config
        .platforms
        .iter_mut()
        .find(|existing| existing.id == platform.id)
    {
        *existing = platform;
    } else {
        config.platforms.push(platform);
    }

    let changed = previous != config.platforms;
    save_runtime_config(&state, service, config)
        .await
        .map_err(map_platform_error)?;
    Ok(Json(ManagementPlatformMutationResponse {
        changed,
        catalog: catalog_response(&state, service).map_err(map_platform_error)?,
    }))
}

pub async fn delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPlatformDeleteRequest>,
) -> Result<Json<ManagementPlatformMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(platform_config_unavailable)?;
    let mut config = service.read_config().map_err(map_platform_error)?;
    let id = non_empty_string(request.id)
        .ok_or_else(|| pipeline_error("platform id is required"))
        .map_err(map_platform_error)?;
    let before = config.platforms.len();
    config.platforms.retain(|platform| platform.id != id);
    let changed = before != config.platforms.len();
    save_runtime_config(&state, service, config)
        .await
        .map_err(map_platform_error)?;
    Ok(Json(ManagementPlatformMutationResponse {
        changed,
        catalog: catalog_response(&state, service).map_err(map_platform_error)?,
    }))
}

pub async fn check(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPlatformCheckRequest>,
) -> Result<Json<ManagementPlatformCheckResponse>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(platform_config_unavailable)?;
    let platform = match request.platform {
        Some(platform) => normalize_platform_value(platform).map_err(map_platform_error)?,
        None => {
            let id = request
                .id
                .and_then(non_empty_string)
                .ok_or_else(|| pipeline_error("platform id is required"))
                .map_err(map_platform_error)?;
            service
                .read_config()
                .map_err(map_platform_error)?
                .platforms
                .into_iter()
                .find(|platform| platform.id == id)
                .ok_or_else(|| pipeline_error(format!("platform {id} is not configured")))
                .map_err(map_platform_error)?
        }
    };
    let result = state
        .platform_health_check()
        .check_platform(platform)
        .await
        .map_err(map_platform_error)?;

    Ok(Json(ManagementPlatformCheckResponse {
        ok: result.ok,
        platform_id: result.platform_id,
        message: result.message,
        status: Some(result.status),
        error_kind: result.error_kind,
        elapsed_ms: Some(result.elapsed_ms),
        webhook_reachable: result.webhook_reachable,
    }))
}

pub async fn legacy_create(
    State(state): State<ManagementApiState>,
    Json(platform): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(platform_config_unavailable)?;
    let mut config = service.read_config().map_err(map_platform_error)?;
    let mut platform = normalize_platform_value(platform).map_err(map_platform_error)?;
    preserve_redacted_platform_secrets(&config.platforms, &mut platform);
    let previous = config.platforms.clone();
    if let Some(existing) = config
        .platforms
        .iter_mut()
        .find(|existing| existing.id == platform.id)
    {
        *existing = platform.clone();
    } else {
        config.platforms.push(platform.clone());
    }
    let changed = previous != config.platforms;
    save_runtime_config(&state, service, config)
        .await
        .map_err(map_platform_error)?;
    Ok(legacy_ok(
        json!({
            "changed": changed,
            "platform": legacy_platform_output(platform),
            "catalog": catalog_response(&state, service).map_err(map_platform_error)?,
        }),
        "平台适配器已保存",
    ))
}

pub async fn legacy_update(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyPlatformUpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(platform_config_unavailable)?;
    let mut config = service.read_config().map_err(map_platform_error)?;
    let id = non_empty_string(request.id)
        .ok_or_else(|| pipeline_error("platform id is required"))
        .map_err(map_platform_error)?;
    let mut platform = normalize_platform_value(request.config).map_err(map_platform_error)?;
    preserve_redacted_platform_secrets(&config.platforms, &mut platform);
    let previous = config.platforms.clone();
    if let Some(existing) = config
        .platforms
        .iter_mut()
        .find(|existing| existing.id == id || existing.id == platform.id)
    {
        *existing = platform.clone();
    } else {
        config.platforms.push(platform.clone());
    }
    let changed = previous != config.platforms;
    save_runtime_config(&state, service, config)
        .await
        .map_err(map_platform_error)?;
    Ok(legacy_ok(
        json!({
            "changed": changed,
            "platform": legacy_platform_output(platform),
            "catalog": catalog_response(&state, service).map_err(map_platform_error)?,
        }),
        "平台适配器已更新",
    ))
}

pub async fn legacy_delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyPlatformDeleteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(platform_config_unavailable)?;
    let mut config = service.read_config().map_err(map_platform_error)?;
    let id = non_empty_string(request.id)
        .ok_or_else(|| pipeline_error("platform id is required"))
        .map_err(map_platform_error)?;
    let before = config.platforms.len();
    config.platforms.retain(|platform| platform.id != id);
    let changed = before != config.platforms.len();
    save_runtime_config(&state, service, config)
        .await
        .map_err(map_platform_error)?;
    Ok(legacy_ok(
        json!({
            "changed": changed,
            "catalog": catalog_response(&state, service).map_err(map_platform_error)?,
        }),
        "平台适配器已删除",
    ))
}

pub async fn legacy_stats(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(platform_config_unavailable)?;
    let config = service.read_config().map_err(map_platform_error)?;
    let stats = if let Some(observability) = state.observability() {
        observability
            .stats_since(None)
            .await
            .map_err(|error| map_platform_error(pipeline_error(error)))?
    } else {
        Vec::new()
    };
    Ok(legacy_ok(
        legacy_platform_stats(&state, &config, stats),
        "获取平台统计信息成功",
    ))
}

pub async fn legacy_webhook(
    State(state): State<ManagementApiState>,
    Path(webhook_uuid): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let service = state
        .config_service()
        .ok_or_else(platform_config_unavailable)?;
    let config = service.read_config().map_err(map_platform_error)?;
    let Some(platform) = config.platforms.into_iter().find(|platform| {
        platform
            .options
            .get("webhook_uuid")
            .and_then(Value::as_str)
            .is_some_and(|value| value == webhook_uuid)
            && (platform
                .options
                .get("unified_webhook")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || webhook_platform_types().contains(&platform.platform_type.as_str()))
    }) else {
        return Err(map_not_found("platform webhook was not found"));
    };
    Ok(legacy_ok(
        json!({
            "webhook_uuid": webhook_uuid,
            "platform_id": platform.id,
            "dispatched": false,
            "message": "webhook endpoint is registered; adapter callback dispatch is handled by runtime platform wiring",
        }),
        "webhook accepted",
    ))
}

fn catalog_response(
    _state: &ManagementApiState,
    service: &RuntimeConfigService,
) -> astrbot_core::Result<ManagementPlatformCatalogResponse> {
    let config = service.read_config()?;
    Ok(ManagementPlatformCatalogResponse {
        summary: platform_summary_from_config(&config),
        platforms: config
            .platforms
            .into_iter()
            .map(ManagementPlatformDescriptor::from)
            .collect(),
        templates: platform_templates(),
    })
}

fn platform_templates() -> Vec<ManagementPlatformTemplate> {
    vec![
        platform_template("Mock", RuntimePlatformConfig::mock("mock")),
        platform_template("WebChat", RuntimePlatformConfig::webchat("webchat")),
        platform_template("Console", RuntimePlatformConfig::console("console")),
        platform_template("OneBot", RuntimePlatformConfig::onebot("onebot")),
        platform_template("aiocqhttp", RuntimePlatformConfig::aiocqhttp("aiocqhttp")),
        platform_template(
            "Telegram",
            RuntimePlatformConfig::new("telegram", TELEGRAM_PLATFORM_TYPE)
                .with_secret("telegram_token", ""),
        ),
        platform_template(
            "Slack",
            RuntimePlatformConfig::new("slack", SLACK_PLATFORM_TYPE)
                .with_option_string("slack_connection_mode", "socket")
                .with_secret("bot_token", "")
                .with_secret("app_token", "")
                .with_secret("signing_secret", "")
                .with_option_u16("slack_webhook_port", 6197),
        ),
        platform_template(
            "Lark",
            RuntimePlatformConfig::new("lark", LARK_PLATFORM_TYPE)
                .with_option_string("app_id", "")
                .with_secret("app_secret", ""),
        ),
        platform_template(
            "LINE",
            RuntimePlatformConfig::new("line", LINE_PLATFORM_TYPE)
                .with_secret("channel_access_token", "")
                .with_secret("channel_secret", ""),
        ),
        platform_template(
            "WeCom",
            RuntimePlatformConfig::new("wecom", WECOM_PLATFORM_TYPE)
                .with_secret("corpid", "")
                .with_secret("secret", ""),
        ),
        platform_template(
            "WeCom AI Bot",
            RuntimePlatformConfig::wecom_ai_bot_long_connection("wecom-ai", "", ""),
        ),
        platform_template(
            "DingTalk",
            RuntimePlatformConfig::dingtalk("dingtalk", "", ""),
        ),
    ]
}

fn platform_template(label: &str, config: RuntimePlatformConfig) -> ManagementPlatformTemplate {
    ManagementPlatformTemplate {
        platform_type: config.platform_type.clone(),
        label: label.to_string(),
        runtime_supported: true,
        config,
    }
}

fn normalize_platform_value(value: Value) -> astrbot_core::Result<RuntimePlatformConfig> {
    let mut object = value
        .as_object()
        .cloned()
        .ok_or_else(|| pipeline_error("platform config must be an object"))?;
    let id = take_string(&mut object, "id").ok_or_else(|| pipeline_error("id is required"))?;
    let platform_type = take_string(&mut object, "type")
        .or_else(|| take_string(&mut object, "platform_type"))
        .ok_or_else(|| pipeline_error("type is required"))?;
    let enabled = take_bool(&mut object, "enabled")
        .or_else(|| take_bool(&mut object, "enable"))
        .unwrap_or(true);
    let name = take_string(&mut object, "name");
    let mut options = take_object(&mut object, "options");
    let mut secrets = take_secret_object(&mut object, "secrets");

    for key in platform_secret_keys() {
        if let Some(value) = object.remove(*key)
            && let Some(secret) = value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        {
            secrets.insert((*key).to_string(), secret.to_string());
        }
    }

    for (key, value) in object {
        if !value.is_null() {
            options.insert(key, value);
        }
    }

    let mut platform = RuntimePlatformConfig {
        id,
        platform_type,
        enabled,
        name,
        options,
        secrets,
    };
    platform.id = non_empty_string(platform.id).ok_or_else(|| pipeline_error("id is required"))?;
    platform.platform_type = non_empty_string(platform.platform_type)
        .ok_or_else(|| pipeline_error("type is required"))?;
    platform.name = platform.name.and_then(non_empty_string);
    let wants_webhook = platform
        .options
        .get("unified_webhook")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || webhook_platform_types().contains(&platform.platform_type.as_str());
    if wants_webhook && !platform.options.contains_key("webhook_uuid") {
        platform.options.insert(
            "webhook_uuid".to_string(),
            Value::String(generated_webhook_uuid(&platform.id)),
        );
    }
    Ok(platform)
}

pub fn legacy_platform_config_value(config: RuntimeConfig) -> Value {
    let mut value = serde_json::to_value(config).unwrap_or_else(|_| json!({}));
    if let Some(object) = value.as_object_mut() {
        let platforms = object
            .get("platforms")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(legacy_platform_value_output)
            .collect::<Vec<_>>();
        object.insert("platform".to_string(), Value::Array(platforms.clone()));
        object.insert("platforms".to_string(), Value::Array(platforms));
    }
    value
}

pub fn legacy_platform_metadata_group() -> Value {
    json!({
        "name": "Platforms",
        "metadata": {
            "platform": {
                "type": "object",
                "description": "Platform adapters",
                "items": {
                    "id": { "type": "string", "hint": "平台适配器 ID" },
                    "type": { "type": "string", "hint": "平台类型" },
                    "enable": { "type": "bool", "hint": "是否启用" },
                    "name": { "type": "string", "hint": "显示名称" },
                    "ws_reverse_host": { "type": "string", "hint": "OneBot reverse WebSocket host" },
                    "ws_reverse_port": { "type": "number", "hint": "OneBot reverse WebSocket port" },
                    "ws_reverse_token": { "type": "password", "hint": "OneBot reverse WebSocket token" },
                    "webhook_uuid": { "type": "string", "hint": "统一 Webhook UUID" }
                },
                "config_template": legacy_platform_template_map()
            }
        }
    })
}

fn legacy_platform_template_map() -> Value {
    let mut map = Map::new();
    for template in platform_templates() {
        map.insert(
            template.label,
            legacy_platform_value_output(
                serde_json::to_value(template.config).unwrap_or(Value::Null),
            ),
        );
    }
    Value::Object(map)
}

fn legacy_platform_output(platform: RuntimePlatformConfig) -> Value {
    legacy_platform_value_output(serde_json::to_value(platform).unwrap_or(Value::Null))
}

fn legacy_platform_value_output(mut platform: Value) -> Value {
    let Some(object) = platform.as_object_mut() else {
        return platform;
    };
    let enabled = object.get("enabled").cloned().unwrap_or(Value::Bool(true));
    object.insert("enable".to_string(), enabled);
    if let Some(options) = object.get("options").and_then(Value::as_object).cloned() {
        for (key, value) in options {
            object.entry(key).or_insert(value);
        }
    }
    if let Some(secrets) = object.get("secrets").and_then(Value::as_object).cloned() {
        let mut redacted_secrets = Map::new();
        for (key, value) in secrets {
            if value.as_str().is_some_and(|secret| !secret.is_empty()) {
                redacted_secrets.insert(key.clone(), Value::String(REDACTED_SECRET.to_string()));
                object.insert(key, Value::String(REDACTED_SECRET.to_string()));
            } else {
                redacted_secrets.insert(key.clone(), value.clone());
                object.entry(key).or_insert(value);
            }
        }
        object.insert("secrets".to_string(), Value::Object(redacted_secrets));
    }
    platform
}

fn legacy_platform_stats(
    state: &ManagementApiState,
    config: &RuntimeConfig,
    stats: Vec<PlatformStatsRecord>,
) -> Value {
    let mut platform_counts = BTreeMap::<String, i64>::new();
    for record in stats {
        *platform_counts.entry(record.platform_id).or_default() += record.count;
    }
    let running_ids = state
        .platforms()
        .platform_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let platforms = config
        .platforms
        .iter()
        .map(|platform| {
            let status = if !platform.enabled {
                "stopped"
            } else if running_ids.contains(&platform.id) {
                "running"
            } else {
                "pending"
            };
            let webhook_uuid = platform
                .options
                .get("webhook_uuid")
                .and_then(Value::as_str)
                .unwrap_or_default();
            json!({
                "id": platform.id,
                "type": platform.platform_type,
                "status": status,
                "error_count": 0,
                "last_error": Value::Null,
                "message_count": platform_counts.get(&platform.id).copied().unwrap_or(0),
                "unified_webhook": !webhook_uuid.is_empty() || webhook_platform_types().contains(&platform.platform_type.as_str()),
                "webhook_uuid": webhook_uuid,
            })
        })
        .collect::<Vec<_>>();
    let running = platforms
        .iter()
        .filter(|platform| platform["status"] == "running")
        .count();
    let error = platforms
        .iter()
        .filter(|platform| platform["status"] == "error")
        .count();
    json!({
        "platforms": platforms,
        "summary": {
            "total": config.platforms.len(),
            "running": running,
            "error": error,
            "total_errors": 0,
        }
    })
}

fn platform_summary_from_config(config: &RuntimeConfig) -> PlatformManagementResponse {
    let enabled_platforms = config
        .platforms
        .iter()
        .filter(|platform| platform.enabled)
        .collect::<Vec<_>>();
    let mut platform_ids = enabled_platforms
        .iter()
        .map(|platform| platform.id.clone())
        .collect::<Vec<_>>();
    platform_ids.sort();
    PlatformManagementResponse {
        platform_count: enabled_platforms.len(),
        platform_ids,
        mock_platform_count: count_platform_type(&enabled_platforms, MOCK_PLATFORM_TYPE),
        webchat_platform_count: count_platform_type(&enabled_platforms, WEBCHAT_PLATFORM_TYPE),
        onebot_platform_count: count_platform_type(&enabled_platforms, ONEBOT_PLATFORM_TYPE)
            + count_platform_type(&enabled_platforms, AIOCQHTTP_PLATFORM_TYPE),
        recording_sink_count: enabled_platforms
            .iter()
            .filter(|platform| {
                matches!(
                    platform.platform_type.as_str(),
                    MOCK_PLATFORM_TYPE
                        | CONSOLE_PLATFORM_TYPE
                        | WEBCHAT_PLATFORM_TYPE
                        | ONEBOT_PLATFORM_TYPE
                        | AIOCQHTTP_PLATFORM_TYPE
                )
            })
            .count(),
    }
}

fn count_platform_type(platforms: &[&RuntimePlatformConfig], platform_type: &str) -> usize {
    platforms
        .iter()
        .filter(|platform| platform.platform_type == platform_type)
        .count()
}

fn preserve_redacted_platform_secrets(
    existing_platforms: &[RuntimePlatformConfig],
    platform: &mut RuntimePlatformConfig,
) {
    if let Some(existing) = existing_platforms
        .iter()
        .find(|existing| existing.id == platform.id)
    {
        for (key, value) in &mut platform.secrets {
            if value == REDACTED_SECRET
                && let Some(existing_value) = existing.secrets.get(key)
            {
                *value = existing_value.clone();
            }
        }
        for (key, value) in &existing.secrets {
            platform
                .secrets
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
    }
}

fn redact_platform_secrets(secrets: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    secrets
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, _)| (key.clone(), REDACTED_SECRET.to_string()))
        .collect()
}

fn take_string(object: &mut Map<String, Value>, key: &str) -> Option<String> {
    object
        .remove(key)
        .and_then(|value| value.as_str().map(str::trim).map(str::to_string))
        .filter(|value| !value.is_empty())
}

fn take_bool(object: &mut Map<String, Value>, key: &str) -> Option<bool> {
    object.remove(key).and_then(|value| value.as_bool())
}

fn take_object(object: &mut Map<String, Value>, key: &str) -> BTreeMap<String, Value> {
    object
        .remove(key)
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn take_secret_object(object: &mut Map<String, Value>, key: &str) -> BTreeMap<String, String> {
    object
        .remove(key)
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(key, value)| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| (key, value.to_string()))
        })
        .collect()
}

fn platform_secret_keys() -> &'static [&'static str] {
    &[
        "telegram_token",
        "bot_token",
        "app_token",
        "signing_secret",
        "app_secret",
        "channel_access_token",
        "channel_secret",
        "ws_reverse_token",
        "corpid",
        "secret",
        "wecomaibot_token",
        "wecomaibot_encoding_aes_key",
        "wecomaibot_ws_bot_id",
        "wecomaibot_ws_secret",
        "client_secret",
    ]
}

fn webhook_platform_types() -> &'static [&'static str] {
    &[
        SLACK_PLATFORM_TYPE,
        LARK_PLATFORM_TYPE,
        LINE_PLATFORM_TYPE,
        WECOM_PLATFORM_TYPE,
        WECOM_AI_BOT_PLATFORM_TYPE,
        DINGTALK_PLATFORM_TYPE,
        "qq_official_webhook",
        "weixin_official_account",
    ]
}

fn generated_webhook_uuid(platform_id: &str) -> String {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{platform_id}-{suffix}")
}

async fn check_platform_with_runtime(
    platform: RuntimePlatformConfig,
) -> astrbot_core::Result<ManagementPlatformHealthResult> {
    let platform_id = platform.id.clone();
    let webhook_reachable = platform_webhook_reachable(&platform);
    let (event_tx, _event_rx) = mpsc::channel(1);
    let manager = match PlatformManager::from_configs(
        &PlatformRegistry::with_builtin_platforms(),
        vec![platform.into()],
        PlatformBuildContext::new(event_tx),
    ) {
        Ok(manager) => manager,
        Err(error) => {
            return Ok(ManagementPlatformHealthResult::unavailable(
                platform_id,
                classify_platform_error(&error),
                sanitize_platform_error(&error),
                0,
                webhook_reachable,
            ));
        }
    };
    let Some(adapter) = manager.adapter(&platform_id) else {
        return Ok(ManagementPlatformHealthResult::unavailable(
            platform_id,
            "configuration",
            "platform configuration did not create a runtime adapter",
            0,
            webhook_reachable,
        ));
    };
    let started = Instant::now();
    let result = tokio::time::timeout(Duration::from_millis(250), adapter.run()).await;
    match result {
        Ok(Ok(())) => {
            let _ = manager.terminate().await;
            Ok(ManagementPlatformHealthResult::available(
                platform_id,
                "platform adapter started successfully",
                started.elapsed().as_millis(),
                webhook_reachable,
            ))
        }
        Ok(Err(error)) => {
            let _ = manager.terminate().await;
            Ok(ManagementPlatformHealthResult::unavailable(
                platform_id,
                classify_platform_error(&error),
                sanitize_platform_error(&error),
                started.elapsed().as_millis(),
                webhook_reachable,
            ))
        }
        Err(_) => {
            let _ = manager.terminate().await;
            Ok(ManagementPlatformHealthResult::available(
                platform_id,
                "platform adapter remained running during startup probe",
                started.elapsed().as_millis(),
                webhook_reachable,
            ))
        }
    }
}

fn platform_webhook_reachable(platform: &RuntimePlatformConfig) -> bool {
    platform
        .options
        .get("unified_webhook")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || platform
            .options
            .get("webhook_uuid")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        || webhook_platform_types().contains(&platform.platform_type.as_str())
}

fn classify_platform_error(error: &AstrbotError) -> String {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("timed out") || message.contains("timeout") {
        "timeout".to_string()
    } else if message.contains("requires secret")
        || message.contains("requires option")
        || message.contains("token")
        || message.contains("credential")
        || message.contains("auth")
    {
        "credential".to_string()
    } else if message.contains("bind")
        || message.contains("connect")
        || message.contains("connection")
        || message.contains("webhook")
        || message.contains("websocket")
    {
        "network".to_string()
    } else {
        "platform".to_string()
    }
}

fn sanitize_platform_error(error: &AstrbotError) -> String {
    error.to_string()
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

fn platform_config_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "runtime config service is not configured".to_string(),
        }),
    )
}

fn map_platform_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
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

fn non_empty_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn pipeline_error(message: impl Into<String>) -> astrbot_core::AstrbotError {
    astrbot_core::AstrbotError::Pipeline(message.into())
}

fn legacy_ok(data: Value, message: impl Into<String>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": message.into(),
        "data": data,
    }))
}
