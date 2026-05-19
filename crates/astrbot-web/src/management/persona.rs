use std::sync::Arc;

use astrbot_persona::{
    PersonaDialogTurn, PersonaFolder, PersonaManager, PersonaProfile, PersonaResolveRequest,
    PersonaResolveSource, ResolvedPersona,
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

#[derive(Clone)]
pub struct ManagementPersonaState {
    manager: Arc<PersonaManager>,
}

impl ManagementPersonaState {
    pub fn new(manager: Arc<PersonaManager>) -> Self {
        Self { manager }
    }

    pub fn manager(&self) -> Arc<PersonaManager> {
        self.manager.clone()
    }
}

impl std::fmt::Debug for ManagementPersonaState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementPersonaState")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPersonaListRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_folder_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPersonaUpsertRequest {
    pub id: String,
    pub system_prompt: String,
    #[serde(default)]
    pub begin_dialogs: Vec<PersonaDialogTurn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub sort_order: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPersonaFolderUpsertRequest {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub sort_order: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPersonaResolveRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forced_persona_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_persona_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_default_persona_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPersonaDeleteRequest {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPersonaMoveRequest {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPersonaCloneRequest {
    pub source_id: String,
    pub new_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementPersonaFolderMoveRequest {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<i32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ManagementPersonaReorderRequest {
    #[serde(default)]
    pub persona_ids: Vec<String>,
    #[serde(default)]
    pub folder_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct SourcePersonaListQuery {
    #[serde(default)]
    pub folder_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct SourcePersonaFolderListQuery {
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPersonaCatalogResponse {
    pub personas: Vec<PersonaProfile>,
    pub folders: Vec<PersonaFolder>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPersonaMutationResponse {
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPersonaActionResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona: Option<PersonaProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<PersonaFolder>,
    pub personas: Vec<PersonaProfile>,
    pub folders: Vec<PersonaFolder>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPersonaResolveResponse {
    pub persona_id: Option<String>,
    pub source: String,
    pub profile: Option<PersonaProfile>,
}

impl From<ResolvedPersona> for ManagementPersonaResolveResponse {
    fn from(value: ResolvedPersona) -> Self {
        Self {
            persona_id: value.persona_id,
            source: match value.source {
                PersonaResolveSource::ForcedSession => "forced_session",
                PersonaResolveSource::Conversation => "conversation",
                PersonaResolveSource::ProviderDefault => "provider_default",
                PersonaResolveSource::Default => "default",
                PersonaResolveSource::WebChatDefault => "webchat_default",
                PersonaResolveSource::Disabled => "disabled",
            }
            .to_string(),
            profile: value.profile,
        }
    }
}

pub async fn catalog(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPersonaListRequest>,
) -> Result<Json<ManagementPersonaCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let personas = state.personas().ok_or_else(personas_unavailable)?;
    let manager = personas.manager();
    let personas = if request.folder_id.is_some() {
        manager
            .personas_by_folder(request.folder_id.as_deref())
            .await
            .map_err(persona_error)?
    } else {
        manager.all_personas().await.map_err(persona_error)?
    };
    let folders = if request.parent_folder_id.is_some() {
        manager
            .folders_by_parent(request.parent_folder_id.as_deref())
            .await
            .map_err(persona_error)?
    } else {
        manager.all_folders().await.map_err(persona_error)?
    };

    Ok(Json(ManagementPersonaCatalogResponse { personas, folders }))
}

pub async fn upsert(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPersonaUpsertRequest>,
) -> Result<Json<ManagementPersonaMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let personas = state.personas().ok_or_else(personas_unavailable)?;
    if request.id.trim().is_empty() || request.system_prompt.trim().is_empty() {
        return Err(bad_request("persona id and system_prompt are required"));
    }
    let mut profile = PersonaProfile::new(request.id, request.system_prompt)
        .with_tools(request.tools)
        .with_skills(request.skills)
        .with_sort_order(request.sort_order);
    profile.begin_dialogs = request.begin_dialogs;
    profile.folder_id = request
        .folder_id
        .filter(|folder_id| !folder_id.trim().is_empty());
    if let Some(message) = request.custom_error_message {
        profile = profile.with_custom_error_message(message);
    }
    personas
        .manager()
        .upsert_persona(profile)
        .await
        .map_err(persona_error)?;

    Ok(Json(ManagementPersonaMutationResponse { ok: true }))
}

pub async fn upsert_folder(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPersonaFolderUpsertRequest>,
) -> Result<Json<ManagementPersonaMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let personas = state.personas().ok_or_else(personas_unavailable)?;
    if request.id.trim().is_empty() || request.name.trim().is_empty() {
        return Err(bad_request("folder id and name are required"));
    }
    let mut folder =
        PersonaFolder::new(request.id, request.name).with_sort_order(request.sort_order);
    if let Some(parent_id) = request.parent_id {
        if !parent_id.trim().is_empty() {
            folder = folder.with_parent_id(parent_id);
        }
    }
    if let Some(description) = request.description {
        folder = folder.with_description(description);
    }
    personas
        .manager()
        .upsert_folder(folder)
        .await
        .map_err(persona_error)?;

    Ok(Json(ManagementPersonaMutationResponse { ok: true }))
}

pub async fn resolve(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPersonaResolveRequest>,
) -> Result<Json<ManagementPersonaResolveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let personas = state.personas().ok_or_else(personas_unavailable)?;
    let mut resolve = PersonaResolveRequest::new();
    if let Some(session_id) = request.session_id {
        resolve = resolve.with_session_id(session_id);
    }
    if let Some(platform_name) = request.platform_name {
        resolve = resolve.with_platform_name(platform_name);
    }
    if let Some(persona_id) = request.forced_persona_id {
        resolve = resolve.with_forced_persona_id(persona_id);
    }
    if let Some(persona_id) = request.conversation_persona_id {
        resolve = resolve.with_conversation_persona_id(persona_id);
    }
    if let Some(persona_id) = request.provider_default_persona_id {
        resolve = resolve.with_provider_default_persona_id(persona_id);
    }
    let resolved = personas
        .manager()
        .resolve(&resolve)
        .await
        .map_err(persona_error)?;

    Ok(Json(resolved.into()))
}

pub async fn delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPersonaDeleteRequest>,
) -> Result<Json<ManagementPersonaActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let personas = state.personas().ok_or_else(personas_unavailable)?;
    let id = non_empty(request.id).ok_or_else(|| bad_request("persona id is required"))?;
    let manager = personas.manager();
    let deleted = manager.delete_persona(&id).await.map_err(persona_error)?;
    Ok(Json(
        action_response(&manager, Some(deleted), None, None).await?,
    ))
}

pub async fn delete_folder(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPersonaDeleteRequest>,
) -> Result<Json<ManagementPersonaActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let personas = state.personas().ok_or_else(personas_unavailable)?;
    let id = non_empty(request.id).ok_or_else(|| bad_request("folder id is required"))?;
    let manager = personas.manager();
    let deleted = manager.delete_folder(&id).await.map_err(persona_error)?;
    Ok(Json(
        action_response(&manager, Some(deleted), None, None).await?,
    ))
}

pub async fn move_persona(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPersonaMoveRequest>,
) -> Result<Json<ManagementPersonaActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let personas = state.personas().ok_or_else(personas_unavailable)?;
    let id = non_empty(request.id).ok_or_else(|| bad_request("persona id is required"))?;
    let manager = personas.manager();
    let persona = manager
        .move_persona(&id, request.folder_id, request.sort_order)
        .await
        .map_err(persona_error)?;
    Ok(Json(
        action_response(&manager, None, Some(persona), None).await?,
    ))
}

pub async fn clone_persona(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPersonaCloneRequest>,
) -> Result<Json<ManagementPersonaActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let personas = state.personas().ok_or_else(personas_unavailable)?;
    let source_id =
        non_empty(request.source_id).ok_or_else(|| bad_request("source persona id is required"))?;
    let new_id =
        non_empty(request.new_id).ok_or_else(|| bad_request("new persona id is required"))?;
    let manager = personas.manager();
    let persona = manager
        .clone_persona(&source_id, &new_id, request.folder_id)
        .await
        .map_err(persona_error)?;
    Ok(Json(
        action_response(&manager, None, Some(persona), None).await?,
    ))
}

pub async fn move_folder(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPersonaFolderMoveRequest>,
) -> Result<Json<ManagementPersonaActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let personas = state.personas().ok_or_else(personas_unavailable)?;
    let id = non_empty(request.id).ok_or_else(|| bad_request("folder id is required"))?;
    let manager = personas.manager();
    let folder = manager
        .move_folder(&id, request.parent_id, request.sort_order)
        .await
        .map_err(persona_error)?;
    Ok(Json(
        action_response(&manager, None, None, Some(folder)).await?,
    ))
}

pub async fn reorder(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementPersonaReorderRequest>,
) -> Result<Json<ManagementPersonaActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let personas = state.personas().ok_or_else(personas_unavailable)?;
    let manager = personas.manager();
    let persona_ids = request
        .persona_ids
        .into_iter()
        .filter_map(non_empty)
        .collect::<Vec<_>>();
    let folder_ids = request
        .folder_ids
        .into_iter()
        .filter_map(non_empty)
        .collect::<Vec<_>>();
    if persona_ids.is_empty() && folder_ids.is_empty() {
        return Err(bad_request("persona_ids or folder_ids are required"));
    }
    if !persona_ids.is_empty() {
        manager
            .reorder_personas(&persona_ids)
            .await
            .map_err(persona_error)?;
    }
    if !folder_ids.is_empty() {
        manager
            .reorder_folders(&folder_ids)
            .await
            .map_err(persona_error)?;
    }
    Ok(Json(action_response(&manager, None, None, None).await?))
}

pub async fn legacy_list(
    State(state): State<ManagementApiState>,
    Query(query): Query<SourcePersonaListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let personas = state.personas().ok_or_else(personas_unavailable)?;
    let manager = personas.manager();
    let filter_by_folder = query.folder_id.is_some();
    let folder_id = query.folder_id.and_then(|folder_id| {
        let folder_id = folder_id.trim().to_string();
        (!folder_id.is_empty()).then_some(folder_id)
    });
    let personas = if filter_by_folder {
        manager
            .personas_by_folder(folder_id.as_deref())
            .await
            .map_err(persona_error)?
    } else {
        manager.all_personas().await.map_err(persona_error)?
    };
    Ok(source_ok(json!(
        personas
            .into_iter()
            .map(persona_to_source)
            .collect::<Vec<_>>()
    )))
}

pub async fn legacy_detail(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let persona_id = string_field(&payload, "persona_id")
        .ok_or_else(|| bad_request("persona_id is required"))?;
    let persona = manager
        .persona(&persona_id)
        .await
        .map_err(persona_error)?
        .ok_or_else(|| not_found("persona not found"))?;
    Ok(source_ok(persona_to_source(persona)))
}

pub async fn legacy_create(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let persona = source_persona_from_payload(&payload, None)?;
    manager
        .upsert_persona(persona.clone())
        .await
        .map_err(persona_error)?;
    Ok(source_ok(json!({
        "message": "人格创建成功",
        "persona": persona_to_source(persona)
    })))
}

pub async fn legacy_update(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let persona_id = string_field(&payload, "persona_id")
        .ok_or_else(|| bad_request("persona_id is required"))?;
    let existing = manager
        .persona(&persona_id)
        .await
        .map_err(persona_error)?
        .ok_or_else(|| not_found("persona not found"))?;
    let persona = source_persona_from_payload(&payload, Some(existing))?;
    manager
        .upsert_persona(persona)
        .await
        .map_err(persona_error)?;
    Ok(source_ok(json!({ "message": "人格更新成功" })))
}

pub async fn legacy_delete(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let persona_id = string_field(&payload, "persona_id")
        .ok_or_else(|| bad_request("persona_id is required"))?;
    manager
        .delete_persona(&persona_id)
        .await
        .map_err(persona_error)?;
    Ok(source_ok(json!({ "message": "人格删除成功" })))
}

pub async fn legacy_clone(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let source_id = string_field(&payload, "source_persona_id")
        .or_else(|| string_field(&payload, "source_id"))
        .ok_or_else(|| bad_request("source_persona_id is required"))?;
    let new_id = string_field(&payload, "new_persona_id")
        .or_else(|| string_field(&payload, "new_id"))
        .ok_or_else(|| bad_request("new_persona_id is required"))?;
    let persona = manager
        .clone_persona(&source_id, &new_id, string_field(&payload, "folder_id"))
        .await
        .map_err(persona_error)?;
    Ok(source_ok(json!({
        "message": "人格克隆成功",
        "persona": persona_to_source(persona)
    })))
}

pub async fn legacy_move(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let persona_id = string_field(&payload, "persona_id")
        .ok_or_else(|| bad_request("persona_id is required"))?;
    manager
        .move_persona(&persona_id, string_field(&payload, "folder_id"), None)
        .await
        .map_err(persona_error)?;
    Ok(source_ok(json!({ "message": "人格移动成功" })))
}

pub async fn legacy_reorder(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let mut persona_ids = Vec::new();
    let mut folder_ids = Vec::new();
    if let Some(items) = payload.get("items").and_then(Value::as_array) {
        for item in items {
            let id = string_field(item, "id")
                .or_else(|| string_field(item, "persona_id"))
                .or_else(|| string_field(item, "folder_id"));
            match (
                item.get("type").and_then(Value::as_str),
                item.get("item_type").and_then(Value::as_str),
                id,
            ) {
                (Some("folder"), _, Some(id)) | (_, Some("folder"), Some(id)) => {
                    folder_ids.push(id)
                }
                (_, _, Some(id)) => persona_ids.push(id),
                _ => {}
            }
        }
    }
    if persona_ids.is_empty() {
        persona_ids = string_vec_field(&payload, "persona_ids");
    }
    if folder_ids.is_empty() {
        folder_ids = string_vec_field(&payload, "folder_ids");
    }
    if !persona_ids.is_empty() {
        manager
            .reorder_personas(&persona_ids)
            .await
            .map_err(persona_error)?;
    }
    if !folder_ids.is_empty() {
        manager
            .reorder_folders(&folder_ids)
            .await
            .map_err(persona_error)?;
    }
    Ok(source_ok(json!({ "message": "排序更新成功" })))
}

pub async fn legacy_folder_list(
    State(state): State<ManagementApiState>,
    Query(query): Query<SourcePersonaFolderListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let parent_id = query.parent_id.and_then(|parent_id| {
        let parent_id = parent_id.trim().to_string();
        (!parent_id.is_empty()).then_some(parent_id)
    });
    let folders = manager
        .folders_by_parent(parent_id.as_deref())
        .await
        .map_err(persona_error)?;
    Ok(source_ok(json!(
        folders
            .into_iter()
            .map(folder_to_source)
            .collect::<Vec<_>>()
    )))
}

pub async fn legacy_folder_tree(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let folders = manager.all_folders().await.map_err(persona_error)?;
    Ok(source_ok(json!(folders_to_source_tree(&folders, None))))
}

pub async fn legacy_folder_detail(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let folder_id =
        string_field(&payload, "folder_id").ok_or_else(|| bad_request("folder_id is required"))?;
    let folder = manager
        .folder(&folder_id)
        .await
        .map_err(persona_error)?
        .ok_or_else(|| not_found("folder not found"))?;
    Ok(source_ok(folder_to_source(folder)))
}

pub async fn legacy_folder_create(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let id = string_field(&payload, "folder_id")
        .or_else(|| string_field(&payload, "id"))
        .unwrap_or_else(|| format!("folder-{}", now_millis()));
    let name = string_field(&payload, "name").ok_or_else(|| bad_request("name is required"))?;
    let mut folder =
        PersonaFolder::new(id, name).with_sort_order(i32_field(&payload, "sort_order"));
    if let Some(parent_id) = string_field(&payload, "parent_id") {
        folder = folder.with_parent_id(parent_id);
    }
    if let Some(description) = string_field(&payload, "description") {
        folder = folder.with_description(description);
    }
    manager
        .upsert_folder(folder.clone())
        .await
        .map_err(persona_error)?;
    Ok(source_ok(json!({
        "message": "文件夹创建成功",
        "folder": folder_to_source(folder)
    })))
}

pub async fn legacy_folder_update(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let folder_id =
        string_field(&payload, "folder_id").ok_or_else(|| bad_request("folder_id is required"))?;
    let mut folder = manager
        .folder(&folder_id)
        .await
        .map_err(persona_error)?
        .ok_or_else(|| not_found("folder not found"))?;
    if let Some(name) = string_field(&payload, "name") {
        folder.name = name;
    }
    if payload.get("parent_id").is_some() {
        folder.parent_id = string_field(&payload, "parent_id");
    }
    if payload.get("description").is_some() {
        folder.description = string_field(&payload, "description");
    }
    if let Some(sort_order) = payload
        .get("sort_order")
        .and_then(Value::as_i64)
        .map(|value| value as i32)
    {
        folder.sort_order = sort_order;
    }
    manager.upsert_folder(folder).await.map_err(persona_error)?;
    Ok(source_ok(json!({ "message": "文件夹更新成功" })))
}

pub async fn legacy_folder_delete(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let manager = state.personas().ok_or_else(personas_unavailable)?.manager();
    let folder_id =
        string_field(&payload, "folder_id").ok_or_else(|| bad_request("folder_id is required"))?;
    manager
        .delete_folder(&folder_id)
        .await
        .map_err(persona_error)?;
    Ok(source_ok(json!({ "message": "文件夹删除成功" })))
}

async fn action_response(
    manager: &PersonaManager,
    deleted: Option<bool>,
    persona: Option<PersonaProfile>,
    folder: Option<PersonaFolder>,
) -> Result<ManagementPersonaActionResponse, (StatusCode, Json<ErrorResponse>)> {
    Ok(ManagementPersonaActionResponse {
        ok: true,
        deleted,
        persona,
        folder,
        personas: manager.all_personas().await.map_err(persona_error)?,
        folders: manager.all_folders().await.map_err(persona_error)?,
    })
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn source_persona_from_payload(
    payload: &Value,
    existing: Option<PersonaProfile>,
) -> Result<PersonaProfile, (StatusCode, Json<ErrorResponse>)> {
    let id = string_field(payload, "persona_id")
        .or_else(|| string_field(payload, "id"))
        .or_else(|| existing.as_ref().map(|persona| persona.id.clone()))
        .ok_or_else(|| bad_request("persona_id is required"))?;
    let system_prompt = string_field(payload, "system_prompt")
        .or_else(|| {
            existing
                .as_ref()
                .map(|persona| persona.system_prompt.clone())
        })
        .ok_or_else(|| bad_request("system_prompt is required"))?;
    let mut persona = PersonaProfile::new(id, system_prompt)
        .with_tools(if let Some(value) = payload.get("tools") {
            string_vec_optional_value(value)
        } else {
            existing.as_ref().and_then(|persona| persona.tools.clone())
        })
        .with_skills(if let Some(value) = payload.get("skills") {
            string_vec_optional_value(value)
        } else {
            existing.as_ref().and_then(|persona| persona.skills.clone())
        })
        .with_sort_order(
            payload
                .get("sort_order")
                .and_then(Value::as_i64)
                .map(|value| value as i32)
                .or_else(|| existing.as_ref().map(|persona| persona.sort_order))
                .unwrap_or_default(),
        );
    persona.begin_dialogs = if let Some(value) = payload.get("begin_dialogs") {
        source_begin_dialogs(value)?
    } else {
        existing
            .as_ref()
            .map(|persona| persona.begin_dialogs.clone())
            .unwrap_or_default()
    };
    persona.folder_id = if payload.get("folder_id").is_some() {
        string_field(payload, "folder_id")
    } else {
        existing
            .as_ref()
            .and_then(|persona| persona.folder_id.clone())
    };
    persona.custom_error_message = if payload.get("custom_error_message").is_some() {
        string_field(payload, "custom_error_message")
    } else {
        existing.and_then(|persona| persona.custom_error_message)
    };
    Ok(persona)
}

fn persona_to_source(persona: PersonaProfile) -> Value {
    json!({
        "persona_id": persona.id,
        "system_prompt": persona.system_prompt,
        "begin_dialogs": source_begin_dialog_strings(&persona.begin_dialogs),
        "tools": optional_string_vec_to_value(persona.tools),
        "skills": optional_string_vec_to_value(persona.skills),
        "custom_error_message": persona.custom_error_message,
        "folder_id": persona.folder_id,
        "sort_order": persona.sort_order,
        "created_at": Value::Null,
        "updated_at": Value::Null
    })
}

fn folder_to_source(folder: PersonaFolder) -> Value {
    json!({
        "folder_id": folder.id,
        "name": folder.name,
        "parent_id": folder.parent_id,
        "description": folder.description,
        "sort_order": folder.sort_order,
        "created_at": Value::Null,
        "updated_at": Value::Null
    })
}

fn folders_to_source_tree(folders: &[PersonaFolder], parent_id: Option<&str>) -> Vec<Value> {
    let mut children = folders
        .iter()
        .filter(|folder| folder.parent_id.as_deref() == parent_id)
        .cloned()
        .collect::<Vec<_>>();
    children.sort_by(compare_source_folders);
    children
        .into_iter()
        .map(|folder| {
            let folder_id = folder.id.clone();
            let mut value = folder_to_source(folder);
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "children".to_string(),
                    Value::Array(folders_to_source_tree(folders, Some(&folder_id))),
                );
            }
            value
        })
        .collect()
}

fn compare_source_folders(left: &PersonaFolder, right: &PersonaFolder) -> std::cmp::Ordering {
    left.sort_order
        .cmp(&right.sort_order)
        .then_with(|| left.name.cmp(&right.name))
}

fn source_begin_dialogs(
    value: &Value,
) -> Result<Vec<PersonaDialogTurn>, (StatusCode, Json<ErrorResponse>)> {
    let Some(items) = value.as_array() else {
        return Err(bad_request("begin_dialogs is invalid"));
    };
    let mut dialogs = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if let Some(content) = item.as_str() {
            let content = content.trim();
            if !content.is_empty() {
                dialogs.push(PersonaDialogTurn::new(
                    if index % 2 == 0 {
                        astrbot_persona::PersonaDialogRole::User
                    } else {
                        astrbot_persona::PersonaDialogRole::Assistant
                    },
                    content,
                ));
            }
            continue;
        }
        let turn = serde_json::from_value(item.clone())
            .map_err(|_| bad_request("begin_dialogs is invalid"))?;
        dialogs.push(turn);
    }
    Ok(dialogs)
}

fn source_begin_dialog_strings(dialogs: &[PersonaDialogTurn]) -> Vec<String> {
    dialogs
        .iter()
        .map(|dialog| dialog.content.clone())
        .collect()
}

fn optional_string_vec_to_value(value: Option<Vec<String>>) -> Value {
    value.map(Value::from).unwrap_or(Value::Null)
}

fn string_field(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn string_vec_field(payload: &Value, key: &str) -> Vec<String> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn string_vec_optional_value(value: &Value) -> Option<Vec<String>> {
    if value.is_null() {
        return None;
    }
    Some(
        value
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    )
}

fn i32_field(payload: &Value, key: &str) -> i32 {
    payload
        .get(key)
        .and_then(Value::as_i64)
        .map(|value| value as i32)
        .unwrap_or_default()
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": "",
        "data": data
    }))
}

fn not_found(message: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
}

fn personas_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "persona management state is not configured".to_string(),
        }),
    )
}

fn bad_request(message: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
}

fn persona_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    let message = error.to_string();
    let status = if message.contains("not found") {
        StatusCode::NOT_FOUND
    } else if message.contains("already exists") {
        StatusCode::CONFLICT
    } else if message.contains("required") {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, Json(ErrorResponse { error: message }))
}
