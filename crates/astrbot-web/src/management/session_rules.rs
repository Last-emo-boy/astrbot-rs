use std::sync::Arc;

use astrbot_session::{
    ProviderCapability, SessionBatchScope, SessionGroup, SessionGroupPatch,
    SessionProviderPreference, SessionRule, SessionRuleKey, SessionRuleValue,
    SessionServiceRulePatch,
};
use astrbot_storage::{SessionGroupRepository, SessionRuleRepository};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

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

fn not_found() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "session group not found".to_string(),
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
