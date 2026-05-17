use astrbot_conversation::{ChatProjectDraft, ChatProjectPatch, ChatProjectService};
use astrbot_storage::{ChatProjectRecord, PlatformSessionRecord};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

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

fn chat_projects_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "chat project management state is not configured".to_string(),
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
