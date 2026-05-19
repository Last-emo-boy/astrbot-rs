use astrbot_conversation::{ChatProjectDraft, ChatProjectPatch, ChatProjectService};
use astrbot_storage::{ChatProjectRecord, PlatformSessionRecord};
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
pub struct ManagementChatProjectState {
    service: ChatProjectService,
}

impl ManagementChatProjectState {
    pub fn new(service: ChatProjectService) -> Self {
        Self { service }
    }

    pub fn service(&self) -> &ChatProjectService {
        &self.service
    }
}

impl std::fmt::Debug for ManagementChatProjectState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementChatProjectState")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementChatProjectCreateRequest {
    pub creator: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub now: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementChatProjectUpdateRequest {
    pub actor: String,
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub now: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementChatProjectActorRequest {
    pub actor: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementChatProjectGetRequest {
    pub actor: String,
    pub project_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementChatProjectMembershipRequest {
    pub actor: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementChatProjectSessionUpsertRequest {
    pub session_id: String,
    pub platform_id: String,
    pub creator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub is_group: bool,
    pub now: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct SourceChatProjectQuery {
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementChatProjectResponse {
    pub project: ManagementChatProjectDescriptor,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementChatProjectCatalogResponse {
    pub projects: Vec<ManagementChatProjectDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementChatProjectSessionsResponse {
    pub sessions: Vec<ManagementPlatformSessionDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementChatProjectSessionResponse {
    pub session: ManagementPlatformSessionDescriptor,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementChatProjectMutationResponse {
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementChatProjectDescriptor {
    pub project_id: String,
    pub creator: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ChatProjectRecord> for ManagementChatProjectDescriptor {
    fn from(record: ChatProjectRecord) -> Self {
        Self {
            project_id: record.project_id,
            creator: record.creator,
            title: record.title,
            emoji: record.emoji,
            description: record.description,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPlatformSessionDescriptor {
    pub session_id: String,
    pub platform_id: String,
    pub creator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub is_group: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<PlatformSessionRecord> for ManagementPlatformSessionDescriptor {
    fn from(record: PlatformSessionRecord) -> Self {
        Self {
            session_id: record.session_id,
            platform_id: record.platform_id,
            creator: record.creator,
            display_name: record.display_name,
            is_group: record.is_group,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

pub async fn create(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementChatProjectCreateRequest>,
) -> Result<Json<ManagementChatProjectResponse>, (StatusCode, Json<ErrorResponse>)> {
    let projects = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?;
    let draft = ChatProjectDraft::new(request.creator, request.title)
        .map_err(|error| map_project_error(error, StatusCode::BAD_REQUEST))?
        .with_optional_emoji(request.emoji)
        .with_optional_description(request.description);
    let project = projects
        .service()
        .create_project(draft, request.now)
        .await
        .map_err(|error| map_project_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Json(ManagementChatProjectResponse {
        project: project.into(),
    }))
}

pub async fn list(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementChatProjectActorRequest>,
) -> Result<Json<ManagementChatProjectCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let projects = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?;
    let project_records = projects
        .service()
        .list_projects(&request.actor)
        .await
        .map_err(|error| map_project_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Json(ManagementChatProjectCatalogResponse {
        projects: project_records.into_iter().map(Into::into).collect(),
    }))
}

pub async fn get(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementChatProjectGetRequest>,
) -> Result<Json<ManagementChatProjectResponse>, (StatusCode, Json<ErrorResponse>)> {
    let projects = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?;
    let project = projects
        .service()
        .get_project(&request.actor, &request.project_id)
        .await
        .map_err(map_project_access_error)?;
    Ok(Json(ManagementChatProjectResponse {
        project: project.into(),
    }))
}

pub async fn update(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementChatProjectUpdateRequest>,
) -> Result<Json<ManagementChatProjectMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let projects = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?;
    let patch = ChatProjectPatch::new()
        .with_title(request.title)
        .with_emoji(request.emoji)
        .with_description(request.description);
    projects
        .service()
        .update_project(&request.actor, &request.project_id, patch, request.now)
        .await
        .map_err(map_project_access_error)?;
    Ok(Json(ManagementChatProjectMutationResponse { ok: true }))
}

pub async fn delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementChatProjectGetRequest>,
) -> Result<Json<ManagementChatProjectMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let projects = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?;
    projects
        .service()
        .delete_project(&request.actor, &request.project_id)
        .await
        .map_err(map_project_access_error)?;
    Ok(Json(ManagementChatProjectMutationResponse { ok: true }))
}

pub async fn upsert_session(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementChatProjectSessionUpsertRequest>,
) -> Result<Json<ManagementChatProjectSessionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let projects = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?;
    let mut session = PlatformSessionRecord::new(
        request.session_id,
        request.platform_id,
        request.creator,
        request.now.clone(),
    )
    .with_updated_at(request.now);
    if let Some(display_name) = request.display_name {
        session = session.with_display_name(display_name);
    }
    if request.is_group {
        session = session.group();
    }
    projects
        .service()
        .repository()
        .upsert_platform_session(session.clone())
        .await
        .map_err(|error| map_project_error(error, StatusCode::BAD_REQUEST))?;
    Ok(Json(ManagementChatProjectSessionResponse {
        session: session.into(),
    }))
}

pub async fn add_session(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementChatProjectMembershipRequest>,
) -> Result<Json<ManagementChatProjectMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let projects = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?;
    let project_id = request.project_id.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "project_id is required".to_string(),
            }),
        )
    })?;
    projects
        .service()
        .add_session_to_project(&request.actor, &request.session_id, &project_id)
        .await
        .map_err(map_project_access_error)?;
    Ok(Json(ManagementChatProjectMutationResponse { ok: true }))
}

pub async fn remove_session(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementChatProjectMembershipRequest>,
) -> Result<Json<ManagementChatProjectMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let projects = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?;
    projects
        .service()
        .remove_session_from_project(&request.actor, &request.session_id)
        .await
        .map_err(map_project_access_error)?;
    Ok(Json(ManagementChatProjectMutationResponse { ok: true }))
}

pub async fn sessions(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementChatProjectGetRequest>,
) -> Result<Json<ManagementChatProjectSessionsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let projects = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?;
    let sessions = projects
        .service()
        .project_sessions(&request.actor, &request.project_id)
        .await
        .map_err(map_project_access_error)?;
    Ok(Json(ManagementChatProjectSessionsResponse {
        sessions: sessions.into_iter().map(Into::into).collect(),
    }))
}

pub async fn legacy_create(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let creator = source_actor(&payload);
    let title = string_field(&payload, "title").ok_or_else(|| bad_request("title is required"))?;
    let request = ManagementChatProjectCreateRequest {
        creator,
        title,
        emoji: string_field(&payload, "emoji").or_else(|| Some("📁".to_string())),
        description: string_field(&payload, "description"),
        now: now_string(&payload),
    };
    let projects = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?;
    let draft = ChatProjectDraft::new(request.creator, request.title)
        .map_err(|error| map_project_error(error, StatusCode::BAD_REQUEST))?
        .with_optional_emoji(request.emoji)
        .with_optional_description(request.description);
    let project = projects
        .service()
        .create_project(draft, request.now)
        .await
        .map_err(|error| map_project_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(source_ok(project_to_source(project.into())))
}

pub async fn legacy_list(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let projects = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?;
    let project_records = projects
        .service()
        .list_projects("guest")
        .await
        .map_err(|error| map_project_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(source_ok(json!(
        project_records
            .into_iter()
            .map(|record| project_to_source(record.into()))
            .collect::<Vec<_>>()
    )))
}

pub async fn legacy_get(
    State(state): State<ManagementApiState>,
    Query(query): Query<SourceChatProjectQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = query
        .project_id
        .and_then(|project_id| {
            let project_id = project_id.trim().to_string();
            (!project_id.is_empty()).then_some(project_id)
        })
        .ok_or_else(|| bad_request("project_id is required"))?;
    let project = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?
        .service()
        .get_project("guest", &project_id)
        .await
        .map_err(map_project_access_error)?;
    Ok(source_ok(project_to_source(project.into())))
}

pub async fn legacy_update(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = string_field(&payload, "project_id")
        .ok_or_else(|| bad_request("project_id is required"))?;
    let patch = ChatProjectPatch::new()
        .with_title(string_field(&payload, "title"))
        .with_emoji(string_field(&payload, "emoji"))
        .with_description(string_field(&payload, "description"));
    state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?
        .service()
        .update_project("guest", &project_id, patch, now_string(&payload))
        .await
        .map_err(map_project_access_error)?;
    Ok(source_ok(json!({})))
}

pub async fn legacy_delete(
    State(state): State<ManagementApiState>,
    Query(query): Query<SourceChatProjectQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = query
        .project_id
        .and_then(|project_id| {
            let project_id = project_id.trim().to_string();
            (!project_id.is_empty()).then_some(project_id)
        })
        .ok_or_else(|| bad_request("project_id is required"))?;
    state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?
        .service()
        .delete_project("guest", &project_id)
        .await
        .map_err(map_project_access_error)?;
    Ok(source_ok(json!({})))
}

pub async fn legacy_add_session(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let session_id = string_field(&payload, "session_id")
        .ok_or_else(|| bad_request("session_id is required"))?;
    let project_id = string_field(&payload, "project_id")
        .ok_or_else(|| bad_request("project_id is required"))?;
    let projects = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?;
    projects
        .service()
        .add_session_to_project("guest", &session_id, &project_id)
        .await
        .map_err(map_project_access_error)?;
    Ok(source_ok(json!({})))
}

pub async fn legacy_remove_session(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let session_id = string_field(&payload, "session_id")
        .ok_or_else(|| bad_request("session_id is required"))?;
    state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?
        .service()
        .remove_session_from_project("guest", &session_id)
        .await
        .map_err(map_project_access_error)?;
    Ok(source_ok(json!({})))
}

pub async fn legacy_sessions(
    State(state): State<ManagementApiState>,
    Query(query): Query<SourceChatProjectQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = query
        .project_id
        .and_then(|project_id| {
            let project_id = project_id.trim().to_string();
            (!project_id.is_empty()).then_some(project_id)
        })
        .ok_or_else(|| bad_request("project_id is required"))?;
    let sessions = state
        .chat_projects()
        .ok_or_else(chat_projects_unavailable)?
        .service()
        .project_sessions("guest", &project_id)
        .await
        .map_err(map_project_access_error)?;
    Ok(source_ok(json!(
        sessions
            .into_iter()
            .map(|record| session_to_source(record.into()))
            .collect::<Vec<_>>()
    )))
}

fn chat_projects_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "chat project management state is not configured".to_string(),
        }),
    )
}

fn project_to_source(project: ManagementChatProjectDescriptor) -> Value {
    json!({
        "project_id": project.project_id,
        "title": project.title,
        "emoji": project.emoji,
        "description": project.description,
        "created_at": project.created_at,
        "updated_at": project.updated_at
    })
}

fn session_to_source(session: ManagementPlatformSessionDescriptor) -> Value {
    json!({
        "session_id": session.session_id,
        "platform_id": session.platform_id,
        "creator": session.creator,
        "display_name": session.display_name,
        "is_group": session.is_group,
        "created_at": session.created_at,
        "updated_at": session.updated_at
    })
}

fn source_actor(payload: &Value) -> String {
    string_field(payload, "actor")
        .or_else(|| string_field(payload, "creator"))
        .unwrap_or_else(|| "guest".to_string())
}

fn string_field(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn now_string(payload: &Value) -> String {
    string_field(payload, "now").unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": "",
        "data": data
    }))
}

fn bad_request(message: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
}

fn map_project_access_error(
    error: astrbot_core::AstrbotError,
) -> (StatusCode, Json<ErrorResponse>) {
    let message = error.to_string();
    let status = if message.contains("permission denied") {
        StatusCode::FORBIDDEN
    } else if message.contains("not found") {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    map_project_error(error, status)
}

fn map_project_error(
    error: astrbot_core::AstrbotError,
    status: StatusCode,
) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}
