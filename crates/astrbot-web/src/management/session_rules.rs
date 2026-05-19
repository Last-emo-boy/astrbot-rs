use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_session::{
    ProviderCapability, SessionBatchScope, SessionGroup, SessionGroupPatch,
    SessionKnowledgeBaseRule, SessionPluginRule, SessionProviderPreference, SessionRule,
    SessionRuleKey, SessionRuleSet, SessionRuleValue, SessionServiceRule, SessionServiceRulePatch,
};
use astrbot_storage::{SessionGroupRepository, SessionRuleRepository};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone)]
pub struct ManagementSessionRuleState {
    rules: Arc<dyn SessionRuleRepository>,
    groups: Arc<dyn SessionGroupRepository>,
}

impl ManagementSessionRuleState {
    pub fn new(
        rules: Arc<dyn SessionRuleRepository>,
        groups: Arc<dyn SessionGroupRepository>,
    ) -> Self {
        Self { rules, groups }
    }

    pub fn rules(&self) -> Arc<dyn SessionRuleRepository> {
        self.rules.clone()
    }

    pub fn groups(&self) -> Arc<dyn SessionGroupRepository> {
        self.groups.clone()
    }
}

impl std::fmt::Debug for ManagementSessionRuleState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementSessionRuleState")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSessionRuleUpdateRequest {
    pub umo: String,
    pub key: SessionRuleKey,
    pub value: SessionRuleValue,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSessionRuleDeleteRequest {
    pub umo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<SessionRuleKey>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSessionServiceBatchRequest {
    pub scope: SessionBatchScope,
    #[serde(default)]
    pub all_umos: Vec<String>,
    pub patch: SessionServiceRulePatch,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSessionProviderBatchRequest {
    pub scope: SessionBatchScope,
    #[serde(default)]
    pub all_umos: Vec<String>,
    pub capability: ProviderCapability,
    pub provider_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSessionGroupUpsertRequest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub umos: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSessionGroupPatchRequest {
    pub id: String,
    pub patch: SessionGroupPatch,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementSessionGroupDeleteRequest {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceSessionRuleListQuery {
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub page_size: Option<usize>,
    #[serde(default)]
    pub search: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceSessionStatusListQuery {
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub page_size: Option<usize>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub message_type: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceSessionRuleUpdateRequest {
    pub umo: String,
    pub rule_key: String,
    #[serde(default)]
    pub rule_value: Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceSessionRuleDeleteRequest {
    pub umo: String,
    #[serde(default)]
    pub rule_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceSessionBatchDeleteRequest {
    #[serde(default)]
    pub umos: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceSessionServiceBatchRequest {
    #[serde(default)]
    pub umos: Vec<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub llm_enabled: Option<bool>,
    #[serde(default)]
    pub tts_enabled: Option<bool>,
    #[serde(default)]
    pub session_enabled: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceSessionProviderBatchRequest {
    #[serde(default)]
    pub umos: Vec<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub group_id: Option<String>,
    pub provider_type: String,
    #[serde(default)]
    pub provider_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceSessionGroupCreateRequest {
    pub name: String,
    #[serde(default)]
    pub umos: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceSessionGroupUpdateRequest {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub umos: Option<Vec<String>>,
    #[serde(default)]
    pub add_umos: Vec<String>,
    #[serde(default)]
    pub remove_umos: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceSessionGroupDeleteRequest {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementSessionRuleListResponse {
    pub rules: Vec<astrbot_session::SessionRuleSet>,
    pub available_rule_keys: Vec<SessionRuleKey>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementSessionRuleMutationResponse {
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementSessionBatchMutationResponse {
    pub success_count: usize,
    pub failed_count: usize,
    pub failed_umos: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementSessionGroupListResponse {
    pub groups: Vec<ManagementSessionGroupDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementSessionGroupDescriptor {
    pub id: String,
    pub name: String,
    pub umos: Vec<String>,
    pub umo_count: usize,
}

impl From<SessionGroup> for ManagementSessionGroupDescriptor {
    fn from(group: SessionGroup) -> Self {
        Self {
            id: group.id,
            name: group.name,
            umo_count: group.umos.len(),
            umos: group.umos,
        }
    }
}

pub async fn list_rules(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementSessionRuleListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let rules = session_rules
        .rules()
        .list_rule_sets()
        .await
        .map_err(internal_error)?;
    Ok(Json(ManagementSessionRuleListResponse {
        rules,
        available_rule_keys: SessionRuleKey::available_keys(),
    }))
}

pub async fn update_rule(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSessionRuleUpdateRequest>,
) -> Result<Json<ManagementSessionRuleMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let rule = SessionRule::new(request.umo, request.key, request.value).ok_or_else(bad_request)?;
    session_rules
        .rules()
        .upsert_rule(rule)
        .await
        .map_err(internal_error)?;
    Ok(Json(ManagementSessionRuleMutationResponse { ok: true }))
}

pub async fn delete_rule(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSessionRuleDeleteRequest>,
) -> Result<Json<ManagementSessionRuleMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    if let Some(key) = request.key {
        session_rules
            .rules()
            .delete_rule(&request.umo, key)
            .await
            .map_err(internal_error)?;
    } else {
        session_rules
            .rules()
            .delete_rule_set(&request.umo)
            .await
            .map_err(internal_error)?;
    }
    Ok(Json(ManagementSessionRuleMutationResponse { ok: true }))
}

pub async fn batch_update_service(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSessionServiceBatchRequest>,
) -> Result<Json<ManagementSessionBatchMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !request.patch.has_changes() {
        return Err(bad_request());
    }
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let target = session_rules
        .groups()
        .resolve_batch_target(request.scope, request.all_umos)
        .await
        .map_err(internal_error)?;
    if target.is_empty() {
        return Err(bad_request());
    }
    let report = session_rules
        .rules()
        .apply_service_rule_patch(&target.resolved_umos, request.patch)
        .await
        .map_err(internal_error)?;
    Ok(Json(ManagementSessionBatchMutationResponse {
        success_count: report.success_count,
        failed_count: report.failed_count(),
        failed_umos: report.failed_umos,
    }))
}

pub async fn batch_update_provider(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSessionProviderBatchRequest>,
) -> Result<Json<ManagementSessionBatchMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let preference = SessionProviderPreference::new(request.capability, request.provider_id)
        .ok_or_else(bad_request)?;
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let target = session_rules
        .groups()
        .resolve_batch_target(request.scope, request.all_umos)
        .await
        .map_err(internal_error)?;
    if target.is_empty() {
        return Err(bad_request());
    }

    let mut success_count = 0;
    let mut failed_umos = Vec::new();
    for umo in &target.resolved_umos {
        if session_rules
            .rules()
            .set_provider_preference(umo, preference.clone())
            .await
            .is_ok()
        {
            success_count += 1;
        } else {
            failed_umos.push(umo.clone());
        }
    }

    Ok(Json(ManagementSessionBatchMutationResponse {
        success_count,
        failed_count: failed_umos.len(),
        failed_umos,
    }))
}

pub async fn list_groups(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementSessionGroupListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let groups = session_rules
        .groups()
        .list_groups()
        .await
        .map_err(internal_error)?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(Json(ManagementSessionGroupListResponse { groups }))
}

pub async fn upsert_group(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSessionGroupUpsertRequest>,
) -> Result<Json<ManagementSessionRuleMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let group = SessionGroup::new(request.id, request.name)
        .ok_or_else(bad_request)?
        .with_umos(request.umos);
    session_rules
        .groups()
        .upsert_group(group)
        .await
        .map_err(internal_error)?;
    Ok(Json(ManagementSessionRuleMutationResponse { ok: true }))
}

pub async fn patch_group(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSessionGroupPatchRequest>,
) -> Result<Json<ManagementSessionRuleMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let mut group = session_rules
        .groups()
        .group(&request.id)
        .await
        .map_err(internal_error)?
        .ok_or_else(not_found)?;
    request.patch.apply_to(&mut group);
    session_rules
        .groups()
        .upsert_group(group)
        .await
        .map_err(internal_error)?;
    Ok(Json(ManagementSessionRuleMutationResponse { ok: true }))
}

pub async fn delete_group(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementSessionGroupDeleteRequest>,
) -> Result<Json<ManagementSessionRuleMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    session_rules
        .groups()
        .delete_group(&request.id)
        .await
        .map_err(internal_error)?;
    Ok(Json(ManagementSessionRuleMutationResponse { ok: true }))
}

pub async fn source_list_rules(
    State(state): State<ManagementApiState>,
    Query(query): Query<SourceSessionRuleListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let mut rules = session_rules
        .rules()
        .list_rule_sets()
        .await
        .map_err(internal_error)?;
    let search = query.search.unwrap_or_default();
    rules.retain(|rule_set| source_rule_matches(rule_set, &search));

    let total = rules.len();
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(10).clamp(1, 100);
    let start = (page - 1).saturating_mul(page_size);
    let page_rules = rules
        .into_iter()
        .skip(start)
        .take(page_size)
        .map(source_rule_set_value)
        .collect::<Vec<_>>();
    let (chat_providers, stt_providers, tts_providers) = source_provider_options(&state);
    let personas = source_persona_options(&state).await?;
    let available_kbs = source_kb_options(&state).await?;

    Ok(source_ok(json!({
        "rules": page_rules,
        "total": total,
        "page": page,
        "page_size": page_size,
        "available_personas": personas,
        "available_chat_providers": chat_providers,
        "available_stt_providers": stt_providers,
        "available_tts_providers": tts_providers,
        "available_plugins": source_plugin_options(&state),
        "available_kbs": available_kbs,
        "available_rule_keys": source_available_rule_keys(),
    })))
}

pub async fn source_update_rule(
    State(state): State<ManagementApiState>,
    Json(request): Json<SourceSessionRuleUpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let key = source_rule_key(&request.rule_key)?;
    let value = source_rule_value(&key, request.rule_value)?;
    let rule = SessionRule::new(request.umo.clone(), key, value)
        .ok_or_else(|| bad_request_message("umo is required"))?;
    session_rules
        .rules()
        .upsert_rule(rule)
        .await
        .map_err(internal_error)?;
    Ok(source_ok(json!({
        "message": format!("规则 {} 已更新", request.rule_key),
        "umo": request.umo,
    })))
}

pub async fn source_delete_rule(
    State(state): State<ManagementApiState>,
    Json(request): Json<SourceSessionRuleDeleteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let umo =
        non_empty_string(request.umo).ok_or_else(|| bad_request_message("umo is required"))?;
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    if let Some(rule_key) = request.rule_key.and_then(non_empty_string) {
        let key = source_rule_key(&rule_key)?;
        session_rules
            .rules()
            .delete_rule(&umo, key)
            .await
            .map_err(internal_error)?;
        Ok(source_ok(json!({
            "message": format!("规则 {rule_key} 已删除"),
            "umo": umo,
        })))
    } else {
        session_rules
            .rules()
            .delete_rule_set(&umo)
            .await
            .map_err(internal_error)?;
        Ok(source_ok(json!({
            "message": "所有规则已删除",
            "umo": umo,
        })))
    }
}

pub async fn source_batch_delete_rule(
    State(state): State<ManagementApiState>,
    Json(request): Json<SourceSessionBatchDeleteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    if request.umos.is_empty() {
        return Err(bad_request_message("umos is required"));
    }
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let mut deleted_count = 0;
    let mut failed_umos = Vec::new();
    for umo in request.umos.into_iter().filter_map(non_empty_string) {
        match session_rules.rules().delete_rule_set(&umo).await {
            Ok(_) => deleted_count += 1,
            Err(_) => failed_umos.push(umo),
        }
    }

    Ok(source_ok(json!({
        "message": format!("已删除 {deleted_count} 条规则"),
        "deleted_count": deleted_count,
        "failed_umos": failed_umos,
        "failed_count": failed_umos.len(),
    })))
}

pub async fn source_active_umos(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    Ok(source_ok(json!({
        "umos": collect_source_umos(&state).await?,
    })))
}

pub async fn source_list_all_with_status(
    State(state): State<ManagementApiState>,
    Query(query): Query<SourceSessionStatusListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let rule_sets = session_rules
        .rules()
        .list_rule_sets()
        .await
        .map_err(internal_error)?;
    let search = query.search.unwrap_or_default();
    let message_type = query.message_type.unwrap_or_else(|| "all".to_string());
    let platform = query.platform.unwrap_or_default();
    let mut sessions = collect_source_umos(&state)
        .await?
        .into_iter()
        .filter_map(|umo| {
            let rule_set = rule_sets.iter().find(|rule_set| rule_set.umo == umo);
            let status = source_session_status_value(&umo, rule_set);
            source_session_status_matches(&status, &search, &message_type, &platform)
                .then_some(status)
        })
        .collect::<Vec<_>>();
    sessions.sort_by(|left, right| {
        left["umo"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["umo"].as_str().unwrap_or_default())
    });
    let total = sessions.len();
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let start = (page - 1).saturating_mul(page_size);
    let page_sessions = sessions
        .into_iter()
        .skip(start)
        .take(page_size)
        .collect::<Vec<_>>();
    let platforms = collect_source_umos(&state)
        .await?
        .into_iter()
        .filter_map(|umo| parse_umo(&umo).0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let (chat_providers, stt_providers, tts_providers) = source_provider_options(&state);

    Ok(source_ok(json!({
        "sessions": page_sessions,
        "total": total,
        "page": page,
        "page_size": page_size,
        "platforms": platforms,
        "available_chat_providers": chat_providers,
        "available_tts_providers": tts_providers,
        "available_stt_providers": stt_providers,
    })))
}

pub async fn source_batch_update_service(
    State(state): State<ManagementApiState>,
    Json(request): Json<SourceSessionServiceBatchRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let patch = SessionServiceRulePatch {
        session_enabled: request.session_enabled,
        llm_enabled: request.llm_enabled,
        tts_enabled: request.tts_enabled,
    };
    if !patch.has_changes() {
        return Err(bad_request_message("at least one service flag is required"));
    }
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let scope = source_batch_scope(request.scope, request.group_id, request.umos)?;
    let target = session_rules
        .groups()
        .resolve_batch_target(scope, collect_source_umos(&state).await?)
        .await
        .map_err(internal_error)?;
    if target.is_empty() {
        return Err(bad_request_message("no session matched batch scope"));
    }
    let report = session_rules
        .rules()
        .apply_service_rule_patch(&target.resolved_umos, patch)
        .await
        .map_err(internal_error)?;

    Ok(source_ok(json!({
        "message": format!("已更新 {} 个会话", report.success_count),
        "success_count": report.success_count,
        "failed_count": report.failed_count(),
        "failed_umos": report.failed_umos,
    })))
}

pub async fn source_batch_update_provider(
    State(state): State<ManagementApiState>,
    Json(request): Json<SourceSessionProviderBatchRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let capability = source_provider_capability(&request.provider_type)?;
    let provider_id = request.provider_id.and_then(non_empty_string);
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let scope = source_batch_scope(request.scope, request.group_id, request.umos)?;
    let target = session_rules
        .groups()
        .resolve_batch_target(scope, collect_source_umos(&state).await?)
        .await
        .map_err(internal_error)?;
    if target.is_empty() {
        return Err(bad_request_message("no session matched batch scope"));
    }

    let mut success_count = 0;
    let mut failed_umos = Vec::new();
    if let Some(provider_id) = provider_id {
        let preference = SessionProviderPreference::new(capability, provider_id.clone())
            .ok_or_else(|| bad_request_message("provider_id is required"))?;
        for umo in &target.resolved_umos {
            if session_rules
                .rules()
                .set_provider_preference(umo, preference.clone())
                .await
                .is_ok()
            {
                success_count += 1;
            } else {
                failed_umos.push(umo.clone());
            }
        }
    } else {
        for umo in &target.resolved_umos {
            if session_rules
                .rules()
                .delete_rule(umo, SessionRuleKey::Provider(capability))
                .await
                .is_ok()
            {
                success_count += 1;
            } else {
                failed_umos.push(umo.clone());
            }
        }
    }

    Ok(source_ok(json!({
        "message": format!("已更新 {} 个会话的 {}", success_count, capability.preference_key()),
        "success_count": success_count,
        "failed_count": failed_umos.len(),
        "failed_umos": failed_umos,
    })))
}

pub async fn source_list_groups(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    Ok(source_ok(json!({
        "groups": source_groups(&state).await?,
    })))
}

pub async fn source_create_group(
    State(state): State<ManagementApiState>,
    Json(request): Json<SourceSessionGroupCreateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let name = non_empty_string(request.name)
        .ok_or_else(|| bad_request_message("group name is required"))?;
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let group_id = source_group_id(&name);
    let group = SessionGroup::new(group_id, name)
        .ok_or_else(|| bad_request_message("group name is required"))?
        .with_umos(request.umos);
    session_rules
        .groups()
        .upsert_group(group.clone())
        .await
        .map_err(internal_error)?;
    Ok(source_ok(json!({
        "message": format!("分组 '{}' 创建成功", group.name),
        "group": source_group_value(group),
    })))
}

pub async fn source_update_group(
    State(state): State<ManagementApiState>,
    Json(request): Json<SourceSessionGroupUpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let mut group = session_rules
        .groups()
        .group(&request.id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found_message("session group not found"))?;
    SessionGroupPatch {
        name: request.name,
        umos: request.umos,
        add_umos: request.add_umos,
        remove_umos: request.remove_umos,
    }
    .apply_to(&mut group);
    session_rules
        .groups()
        .upsert_group(group.clone())
        .await
        .map_err(internal_error)?;
    Ok(source_ok(json!({
        "message": format!("分组 '{}' 更新成功", group.name),
        "group": source_group_value(group),
    })))
}

pub async fn source_delete_group(
    State(state): State<ManagementApiState>,
    Json(request): Json<SourceSessionGroupDeleteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let group = session_rules
        .groups()
        .group(&request.id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found_message("session group not found"))?;
    session_rules
        .groups()
        .delete_group(&request.id)
        .await
        .map_err(internal_error)?;
    Ok(source_ok(json!({
        "message": format!("分组 '{}' 已删除", group.name),
    })))
}

fn source_rule_set_value(rule_set: SessionRuleSet) -> Value {
    let (platform, message_type, session_id) = parse_umo(&rule_set.umo);
    json!({
        "umo": rule_set.umo,
        "platform": platform.unwrap_or_default(),
        "message_type": message_type.unwrap_or_default(),
        "session_id": session_id.unwrap_or_default(),
        "rules": source_rules_object(&rule_set),
    })
}

fn source_rules_object(rule_set: &SessionRuleSet) -> Value {
    let mut rules = serde_json::Map::new();
    if let Some(service) = &rule_set.service {
        rules.insert("session_service_config".to_string(), json!(service));
    }
    if let Some(plugin) = &rule_set.plugin {
        rules.insert("session_plugin_config".to_string(), json!(plugin));
    }
    if let Some(knowledge_base) = &rule_set.knowledge_base {
        rules.insert("kb_config".to_string(), json!(knowledge_base));
    }
    for provider in &rule_set.provider_preferences {
        rules.insert(
            provider.capability.preference_key().to_string(),
            json!(provider.provider_id),
        );
    }
    Value::Object(rules)
}

fn source_session_status_value(umo: &str, rule_set: Option<&SessionRuleSet>) -> Value {
    let (platform, message_type, session_id) = parse_umo(umo);
    let service = rule_set.and_then(|rules| rules.service.as_ref());
    let provider = |capability| {
        rule_set
            .and_then(|rules| rules.provider_for(capability))
            .unwrap_or_default()
            .to_string()
    };
    json!({
        "umo": umo,
        "platform": platform.unwrap_or_else(|| "unknown".to_string()),
        "message_type": message_type.unwrap_or_else(|| "unknown".to_string()),
        "session_id": session_id.unwrap_or_else(|| umo.to_string()),
        "custom_name": service.and_then(|rule| rule.custom_name.clone()).unwrap_or_default(),
        "session_enabled": service.and_then(|rule| rule.session_enabled).unwrap_or(true),
        "llm_enabled": service.and_then(|rule| rule.llm_enabled).unwrap_or(true),
        "tts_enabled": service.and_then(|rule| rule.tts_enabled).unwrap_or(true),
        "has_rules": rule_set.is_some_and(SessionRuleSet::has_any_rule),
        "chat_provider": provider(ProviderCapability::ChatCompletion),
        "tts_provider": provider(ProviderCapability::TextToSpeech),
        "stt_provider": provider(ProviderCapability::SpeechToText),
    })
}

fn source_session_status_matches(
    status: &Value,
    search: &str,
    message_type: &str,
    platform: &str,
) -> bool {
    if !platform.trim().is_empty()
        && status["platform"].as_str().unwrap_or_default() != platform.trim()
    {
        return false;
    }
    let status_message_type = status["message_type"].as_str().unwrap_or_default();
    match message_type.trim() {
        "" | "all" => {}
        "group" => {
            if !is_group_message_type(status_message_type) {
                return false;
            }
        }
        "private" => {
            if !is_private_message_type(status_message_type) {
                return false;
            }
        }
        other if other != status_message_type => return false,
        _ => {}
    }
    let search = search.trim().to_lowercase();
    search.is_empty()
        || [
            status["umo"].as_str().unwrap_or_default(),
            status["custom_name"].as_str().unwrap_or_default(),
        ]
        .iter()
        .any(|value| value.to_lowercase().contains(&search))
}

fn source_rule_matches(rule_set: &SessionRuleSet, search: &str) -> bool {
    let search = search.trim().to_lowercase();
    if search.is_empty() {
        return true;
    }
    let rules = source_rules_object(rule_set).to_string().to_lowercase();
    rule_set.umo.to_lowercase().contains(&search) || rules.contains(&search)
}

fn source_rule_key(rule_key: &str) -> Result<SessionRuleKey, (StatusCode, Json<ErrorResponse>)> {
    match rule_key.trim() {
        "session_service_config" => Ok(SessionRuleKey::Service),
        "session_plugin_config" => Ok(SessionRuleKey::Plugin),
        "kb_config" => Ok(SessionRuleKey::KnowledgeBase),
        "provider_perf_chat_completion" | "chat_completion" => {
            Ok(SessionRuleKey::Provider(ProviderCapability::ChatCompletion))
        }
        "provider_perf_speech_to_text" | "speech_to_text" => {
            Ok(SessionRuleKey::Provider(ProviderCapability::SpeechToText))
        }
        "provider_perf_text_to_speech" | "text_to_speech" => {
            Ok(SessionRuleKey::Provider(ProviderCapability::TextToSpeech))
        }
        _ => Err(bad_request_message("unsupported session rule key")),
    }
}

fn source_provider_capability(
    provider_type: &str,
) -> Result<ProviderCapability, (StatusCode, Json<ErrorResponse>)> {
    match source_rule_key(provider_type)? {
        SessionRuleKey::Provider(capability) => Ok(capability),
        _ => Err(bad_request_message("unsupported provider type")),
    }
}

fn source_rule_value(
    key: &SessionRuleKey,
    value: Value,
) -> Result<SessionRuleValue, (StatusCode, Json<ErrorResponse>)> {
    match key {
        SessionRuleKey::Service => serde_json::from_value::<SessionServiceRule>(value)
            .map(SessionRuleValue::Service)
            .map_err(|error| bad_request_message(format!("invalid service rule: {error}"))),
        SessionRuleKey::Plugin => serde_json::from_value::<SessionPluginRule>(value)
            .map(SessionRuleValue::Plugin)
            .map_err(|error| bad_request_message(format!("invalid plugin rule: {error}"))),
        SessionRuleKey::KnowledgeBase => serde_json::from_value::<SessionKnowledgeBaseRule>(value)
            .map(SessionRuleValue::KnowledgeBase)
            .map_err(|error| bad_request_message(format!("invalid kb rule: {error}"))),
        SessionRuleKey::Provider(capability) => {
            let provider_id = source_string_value(&value)
                .ok_or_else(|| bad_request_message("provider id is required"))?;
            let preference = SessionProviderPreference::new(*capability, provider_id)
                .ok_or_else(|| bad_request_message("provider id is required"))?;
            Ok(SessionRuleValue::Provider(preference))
        }
    }
}

fn source_string_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => non_empty_string(value.clone()),
        Value::Object(map) => map
            .get("provider_id")
            .or_else(|| map.get("id"))
            .and_then(Value::as_str)
            .and_then(|value| non_empty_string(value.to_string())),
        _ => None,
    }
}

fn source_batch_scope(
    scope: Option<String>,
    group_id: Option<String>,
    umos: Vec<String>,
) -> Result<SessionBatchScope, (StatusCode, Json<ErrorResponse>)> {
    let umos = umos
        .into_iter()
        .filter_map(non_empty_string)
        .collect::<Vec<_>>();
    if !umos.is_empty() {
        return Ok(SessionBatchScope::Explicit(umos));
    }
    match scope.as_deref().map(str::trim) {
        Some("all") => Ok(SessionBatchScope::All),
        Some("group") => Ok(SessionBatchScope::Group),
        Some("private") => Ok(SessionBatchScope::Private),
        Some("custom_group") => group_id
            .and_then(non_empty_string)
            .map(SessionBatchScope::CustomGroup)
            .ok_or_else(|| bad_request_message("group_id is required")),
        _ => Ok(SessionBatchScope::Explicit(Vec::new())),
    }
}

async fn collect_source_umos(
    state: &ManagementApiState,
) -> Result<Vec<String>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    let mut umos = BTreeSet::new();
    for rule_set in session_rules
        .rules()
        .list_rule_sets()
        .await
        .map_err(internal_error)?
    {
        umos.insert(rule_set.umo);
    }
    for group in session_rules
        .groups()
        .list_groups()
        .await
        .map_err(internal_error)?
    {
        umos.extend(group.umos);
    }
    if let Some(conversations) = state.conversations() {
        for record in conversations
            .service()
            .list(None)
            .await
            .map_err(internal_error)?
        {
            umos.insert(record.user_id.unwrap_or_else(|| {
                format!(
                    "{}:FriendMessage:{}",
                    record.platform_id, record.conversation_id
                )
            }));
        }
    }
    Ok(umos.into_iter().collect())
}

async fn source_groups(
    state: &ManagementApiState,
) -> Result<Vec<Value>, (StatusCode, Json<ErrorResponse>)> {
    let session_rules = state
        .session_rules()
        .ok_or_else(session_rules_unavailable)?;
    Ok(session_rules
        .groups()
        .list_groups()
        .await
        .map_err(internal_error)?
        .into_iter()
        .map(source_group_value)
        .collect())
}

fn source_group_value(group: SessionGroup) -> Value {
    json!({
        "id": group.id,
        "name": group.name,
        "umo_count": group.umos.len(),
        "umos": group.umos,
    })
}

fn source_group_id(name: &str) -> String {
    let slug = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!(
        "group-{}-{suffix}",
        if slug.is_empty() { "session" } else { &slug }
    )
}

fn source_provider_options(state: &ManagementApiState) -> (Vec<Value>, Vec<Value>, Vec<Value>) {
    let mut chat = Vec::new();
    let mut stt = Vec::new();
    let mut tts = Vec::new();
    if let Some(service) = state.config_service()
        && let Ok(config) = service.read_config()
    {
        chat = config
            .chat_providers
            .into_iter()
            .map(|provider| source_provider_option(provider.id, provider.model))
            .collect();
        stt = config
            .speech_to_text_providers
            .into_iter()
            .map(|provider| source_provider_option(provider.id, provider.model))
            .collect();
        tts = config
            .text_to_speech_providers
            .into_iter()
            .map(|provider| source_provider_option(provider.id, provider.model))
            .collect();
    }
    if chat.is_empty()
        && let Some(default_id) = &state.providers().default_chat_provider_id
    {
        chat.push(source_provider_option(default_id.clone(), None));
    }
    if stt.is_empty()
        && let Some(default_id) = &state.providers().default_speech_to_text_provider_id
    {
        stt.push(source_provider_option(default_id.clone(), None));
    }
    if tts.is_empty()
        && let Some(default_id) = &state.providers().default_text_to_speech_provider_id
    {
        tts.push(source_provider_option(default_id.clone(), None));
    }
    (chat, stt, tts)
}

fn source_provider_option(id: String, model: Option<String>) -> Value {
    json!({
        "id": id.clone(),
        "name": id,
        "model": model.unwrap_or_default(),
    })
}

async fn source_persona_options(
    state: &ManagementApiState,
) -> Result<Vec<Value>, (StatusCode, Json<ErrorResponse>)> {
    let Some(personas) = state.personas() else {
        return Ok(Vec::new());
    };
    Ok(personas
        .manager()
        .all_personas()
        .await
        .map_err(internal_error)?
        .into_iter()
        .map(|persona| {
            json!({
                "name": persona.id,
                "prompt": persona.system_prompt,
            })
        })
        .collect())
}

async fn source_kb_options(
    state: &ManagementApiState,
) -> Result<Vec<Value>, (StatusCode, Json<ErrorResponse>)> {
    let Some(knowledge_base) = state.knowledge_base() else {
        return Ok(Vec::new());
    };
    Ok(knowledge_base
        .management()
        .list_kbs()
        .await
        .map_err(internal_error)?
        .knowledge_bases
        .into_iter()
        .map(|kb| {
            json!({
                "kb_id": kb.kb_id,
                "kb_name": kb.name,
                "emoji": kb.emoji,
            })
        })
        .collect())
}

fn source_plugin_options(state: &ManagementApiState) -> Vec<Value> {
    state
        .plugins()
        .handlers
        .iter()
        .map(|handler| handler.plugin_name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|name| json!({ "name": name, "display_name": name, "desc": "" }))
        .collect()
}

fn source_available_rule_keys() -> Vec<&'static str> {
    SessionRuleKey::available_keys()
        .into_iter()
        .map(|key| key.storage_key())
        .collect()
}

fn parse_umo(umo: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut parts = umo.splitn(3, ':');
    let platform = parts
        .next()
        .and_then(|part| non_empty_string(part.to_string()));
    let message_type = parts
        .next()
        .and_then(|part| non_empty_string(part.to_string()));
    let session_id = parts
        .next()
        .and_then(|part| non_empty_string(part.to_string()));
    (platform, message_type, session_id)
}

fn is_group_message_type(message_type: &str) -> bool {
    matches!(
        message_type.to_ascii_lowercase().as_str(),
        "group" | "groupmessage"
    )
}

fn is_private_message_type(message_type: &str) -> bool {
    matches!(
        message_type.to_ascii_lowercase().as_str(),
        "private" | "friend" | "friendmessage" | "direct"
    )
}

fn non_empty_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({ "status": "ok", "message": "", "data": data }))
}

fn session_rules_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "session rule management state is not configured".to_string(),
        }),
    )
}

fn bad_request() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "invalid session rule request".to_string(),
        }),
    )
}

fn bad_request_message(message: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}

fn not_found() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "session group not found".to_string(),
        }),
    )
}

fn not_found_message(message: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}

fn internal_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}
