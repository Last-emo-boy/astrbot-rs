use astrbot_kb::{
    ChunkId, DocumentId, InMemoryKnowledgeBaseManagementStore, InMemoryKnowledgeUploadTaskStore,
    KnowledgeBaseCatalog, KnowledgeBaseCreateCommand, KnowledgeBaseId,
    KnowledgeBaseManagementService, KnowledgeBaseSummary, KnowledgeBaseUpdateCommand,
    KnowledgeChunkCatalog, KnowledgeDocumentCatalog, KnowledgeDocumentSummary,
    KnowledgeEmbeddingPreflight, KnowledgeProviderPreflightReport,
    KnowledgeProviderPreflightRequest, KnowledgeProviderPreflightService, KnowledgeRerankPreflight,
    KnowledgeUploadProgress, KnowledgeUploadStage, KnowledgeUploadTaskId, KnowledgeUploadTaskKind,
    KnowledgeUploadTaskResult, KnowledgeUploadTaskService, KnowledgeUploadTaskStatus,
    KnowledgeUploadTaskSummary,
};
use astrbot_provider::ProviderManager;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone, Debug)]
pub struct ManagementKnowledgeBaseState {
    management: KnowledgeBaseManagementService,
    preflight: KnowledgeProviderPreflightService,
    upload_tasks: KnowledgeUploadTaskService,
}

impl ManagementKnowledgeBaseState {
    pub fn new(
        management: KnowledgeBaseManagementService,
        preflight: KnowledgeProviderPreflightService,
        upload_tasks: KnowledgeUploadTaskService,
    ) -> Self {
        Self {
            management,
            preflight,
            upload_tasks,
        }
    }

    pub fn in_memory(provider_manager: ProviderManager) -> Self {
        Self {
            management: KnowledgeBaseManagementService::new(std::sync::Arc::new(
                InMemoryKnowledgeBaseManagementStore::new(),
            )),
            preflight: KnowledgeProviderPreflightService::new(
                std::sync::Arc::new(provider_manager.clone()),
                Some(std::sync::Arc::new(provider_manager)),
            ),
            upload_tasks: KnowledgeUploadTaskService::new(std::sync::Arc::new(
                InMemoryKnowledgeUploadTaskStore::new(),
            )),
        }
    }

    pub fn management(&self) -> &KnowledgeBaseManagementService {
        &self.management
    }

    pub fn preflight(&self) -> &KnowledgeProviderPreflightService {
        &self.preflight
    }

    pub fn upload_tasks(&self) -> &KnowledgeUploadTaskService {
        &self.upload_tasks
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementKnowledgeBaseCreateRequest {
    pub kb_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    pub embedding_provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerank_provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_overlap: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementKnowledgeBaseUpdateRequest {
    pub kb_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerank_provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_overlap: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementKnowledgeBaseIdRequest {
    pub kb_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementKnowledgeDocumentIdRequest {
    pub doc_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementKnowledgeChunkDeleteRequest {
    pub chunk_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementKnowledgeProviderPreflightRequest {
    pub embedding_provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_embedding_dimension: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerank_provider_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementKnowledgeUploadPlanRequest {
    pub task_id: String,
    pub kb_id: String,
    pub kind: KnowledgeUploadTaskKind,
    #[serde(default = "default_file_total")]
    pub file_total: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementKnowledgeUploadProgressRequest {
    pub task_id: String,
    pub file_index: usize,
    pub file_total: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    pub stage: KnowledgeUploadStage,
    pub current: usize,
    pub total: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementKnowledgeUploadCompleteRequest {
    pub task_id: String,
    #[serde(default)]
    pub document_ids: Vec<String>,
    #[serde(default)]
    pub chunk_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementKnowledgeUploadFailRequest {
    pub task_id: String,
    pub error: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementKnowledgeBaseResponse {
    pub knowledge_base: KnowledgeBaseSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementKnowledgeBaseCatalogResponse {
    pub knowledge_bases: Vec<KnowledgeBaseSummary>,
}

impl From<KnowledgeBaseCatalog> for ManagementKnowledgeBaseCatalogResponse {
    fn from(catalog: KnowledgeBaseCatalog) -> Self {
        Self {
            knowledge_bases: catalog.knowledge_bases,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementKnowledgeDocumentCatalogResponse {
    pub documents: Vec<KnowledgeDocumentSummary>,
}

impl From<KnowledgeDocumentCatalog> for ManagementKnowledgeDocumentCatalogResponse {
    fn from(catalog: KnowledgeDocumentCatalog) -> Self {
        Self {
            documents: catalog.documents,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementKnowledgeChunkCatalogResponse {
    pub chunks: Vec<astrbot_kb::KnowledgeChunkSummary>,
}

impl From<KnowledgeChunkCatalog> for ManagementKnowledgeChunkCatalogResponse {
    fn from(catalog: KnowledgeChunkCatalog) -> Self {
        Self {
            chunks: catalog.chunks,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementKnowledgeMutationResponse {
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementKnowledgePreflightResponse {
    pub report: KnowledgeProviderPreflightReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementKnowledgeUploadTaskResponse {
    pub task: KnowledgeUploadTaskSummary,
}

pub async fn catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementKnowledgeBaseCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let catalog = knowledge_base
        .management()
        .list_kbs()
        .await
        .map_err(internal_error)?;
    Ok(Json(catalog.into()))
}

pub async fn create(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeBaseCreateRequest>,
) -> Result<Json<ManagementKnowledgeBaseResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let preflight = knowledge_base
        .preflight()
        .preflight(
            KnowledgeProviderPreflightRequest::new(&request.embedding_provider_id)
                .with_rerank_provider_id(request.rerank_provider_id.clone()),
        )
        .await
        .map_err(bad_request)?;
    if !preflight.is_usable() {
        return Err(preflight_failed(preflight));
    }

    let command = KnowledgeBaseCreateCommand::new(
        KnowledgeBaseId::new(request.kb_id).map_err(bad_request)?,
        request.name,
        request.embedding_provider_id,
    )
    .with_description(request.description)
    .with_emoji(request.emoji)
    .with_rerank_provider_id(request.rerank_provider_id)
    .with_chunking(request.chunk_size, request.chunk_overlap);
    let created = knowledge_base
        .management()
        .create_kb(command)
        .await
        .map_err(bad_request)?;
    Ok(Json(ManagementKnowledgeBaseResponse {
        knowledge_base: created,
    }))
}

pub async fn get(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeBaseIdRequest>,
) -> Result<Json<ManagementKnowledgeBaseResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let kb_id = KnowledgeBaseId::new(request.kb_id).map_err(bad_request)?;
    let knowledge_base = knowledge_base
        .management()
        .get_kb(&kb_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("knowledge base not found"))?;
    Ok(Json(ManagementKnowledgeBaseResponse { knowledge_base }))
}

pub async fn update(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeBaseUpdateRequest>,
) -> Result<Json<ManagementKnowledgeBaseResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let kb_id = KnowledgeBaseId::new(request.kb_id).map_err(bad_request)?;
    let command = KnowledgeBaseUpdateCommand {
        name: request.name,
        description: request.description,
        emoji: request.emoji,
        embedding_provider_id: request.embedding_provider_id,
        rerank_provider_id: request.rerank_provider_id,
        chunk_size: request.chunk_size,
        chunk_overlap: request.chunk_overlap,
        top_k_dense: None,
        top_k_sparse: None,
        top_m_final: None,
    };
    let updated = knowledge_base
        .management()
        .update_kb(&kb_id, command)
        .await
        .map_err(bad_request)?
        .ok_or_else(|| not_found("knowledge base not found"))?;
    Ok(Json(ManagementKnowledgeBaseResponse {
        knowledge_base: updated,
    }))
}

pub async fn delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeBaseIdRequest>,
) -> Result<Json<ManagementKnowledgeMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let kb_id = KnowledgeBaseId::new(request.kb_id).map_err(bad_request)?;
    let deleted = knowledge_base
        .management()
        .delete_kb(&kb_id)
        .await
        .map_err(internal_error)?;
    if !deleted {
        return Err(not_found("knowledge base not found"));
    }
    Ok(Json(ManagementKnowledgeMutationResponse { ok: true }))
}

pub async fn preflight(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeProviderPreflightRequest>,
) -> Result<Json<ManagementKnowledgePreflightResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let mut preflight_request =
        KnowledgeProviderPreflightRequest::new(request.embedding_provider_id)
            .with_rerank_provider_id(request.rerank_provider_id);
    if let Some(dimension) = request.expected_embedding_dimension {
        preflight_request = preflight_request.with_expected_embedding_dimension(dimension);
    }
    let report = knowledge_base
        .preflight()
        .preflight(preflight_request)
        .await
        .map_err(bad_request)?;
    Ok(Json(ManagementKnowledgePreflightResponse { report }))
}

pub async fn list_documents(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeBaseIdRequest>,
) -> Result<Json<ManagementKnowledgeDocumentCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let kb_id = KnowledgeBaseId::new(request.kb_id).map_err(bad_request)?;
    let catalog = knowledge_base
        .management()
        .list_documents(&kb_id)
        .await
        .map_err(internal_error)?;
    Ok(Json(catalog.into()))
}

pub async fn get_document(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeDocumentIdRequest>,
) -> Result<Json<KnowledgeDocumentSummary>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let doc_id = DocumentId::new(request.doc_id).map_err(bad_request)?;
    let document = knowledge_base
        .management()
        .get_document(&doc_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("knowledge document not found"))?;
    Ok(Json(document))
}

pub async fn delete_document(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeDocumentIdRequest>,
) -> Result<Json<ManagementKnowledgeMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let doc_id = DocumentId::new(request.doc_id).map_err(bad_request)?;
    let deleted = knowledge_base
        .management()
        .delete_document(&doc_id)
        .await
        .map_err(internal_error)?;
    if !deleted {
        return Err(not_found("knowledge document not found"));
    }
    Ok(Json(ManagementKnowledgeMutationResponse { ok: true }))
}

pub async fn list_chunks(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeDocumentIdRequest>,
) -> Result<Json<ManagementKnowledgeChunkCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let doc_id = DocumentId::new(request.doc_id).map_err(bad_request)?;
    let catalog = knowledge_base
        .management()
        .list_chunks_for_document(&doc_id)
        .await
        .map_err(internal_error)?;
    Ok(Json(catalog.into()))
}

pub async fn delete_chunk(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeChunkDeleteRequest>,
) -> Result<Json<ManagementKnowledgeMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let chunk_id = ChunkId::new(request.chunk_id).map_err(bad_request)?;
    let deleted = knowledge_base
        .management()
        .delete_chunk(&chunk_id)
        .await
        .map_err(internal_error)?;
    if !deleted {
        return Err(not_found("knowledge chunk not found"));
    }
    Ok(Json(ManagementKnowledgeMutationResponse { ok: true }))
}

pub async fn plan_upload(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeUploadPlanRequest>,
) -> Result<Json<ManagementKnowledgeUploadTaskResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let task = knowledge_base
        .upload_tasks()
        .start_task(
            KnowledgeUploadTaskId::new(request.task_id).map_err(bad_request)?,
            request.kind,
            request.kb_id,
            request.file_total,
        )
        .await
        .map_err(internal_error)?;
    Ok(Json(ManagementKnowledgeUploadTaskResponse { task }))
}

pub async fn update_upload_progress(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeUploadProgressRequest>,
) -> Result<Json<ManagementKnowledgeUploadTaskResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let task_id = KnowledgeUploadTaskId::new(request.task_id).map_err(bad_request)?;
    let progress = KnowledgeUploadProgress::queued(request.file_total).processing(
        request.file_index,
        request.file_name.unwrap_or_default(),
        request.stage,
        request.current,
        request.total,
    );
    let task = knowledge_base
        .upload_tasks()
        .update_progress(&task_id, progress)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("knowledge upload task not found"))?;
    Ok(Json(ManagementKnowledgeUploadTaskResponse { task }))
}

pub async fn complete_upload(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeUploadCompleteRequest>,
) -> Result<Json<ManagementKnowledgeUploadTaskResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let task_id = KnowledgeUploadTaskId::new(request.task_id).map_err(bad_request)?;
    let task = knowledge_base
        .upload_tasks()
        .complete_task(
            &task_id,
            KnowledgeUploadTaskResult::new(request.document_ids, request.chunk_count),
        )
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("knowledge upload task not found"))?;
    Ok(Json(ManagementKnowledgeUploadTaskResponse { task }))
}

pub async fn fail_upload(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeUploadFailRequest>,
) -> Result<Json<ManagementKnowledgeUploadTaskResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let task_id = KnowledgeUploadTaskId::new(request.task_id).map_err(bad_request)?;
    let task = knowledge_base
        .upload_tasks()
        .fail_task(&task_id, request.error)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("knowledge upload task not found"))?;
    Ok(Json(ManagementKnowledgeUploadTaskResponse { task }))
}

pub async fn upload_progress(
    State(state): State<ManagementApiState>,
    Path(task_id): Path<String>,
) -> Result<Json<ManagementKnowledgeUploadTaskResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let task_id = KnowledgeUploadTaskId::new(task_id).map_err(bad_request)?;
    let task = knowledge_base
        .upload_tasks()
        .task(&task_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("knowledge upload task not found"))?;
    Ok(Json(ManagementKnowledgeUploadTaskResponse { task }))
}

fn default_file_total() -> usize {
    1
}

fn knowledge_base_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "knowledge base management state is not configured".to_string(),
        }),
    )
}

fn bad_request(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn preflight_failed(report: KnowledgeProviderPreflightReport) -> (StatusCode, Json<ErrorResponse>) {
    let message = if let Some(error) = report.embedding.error {
        error
    } else if !report.embedding.dimension_matches {
        format!(
            "embedding dimension mismatch: expected {:?}, actual {:?}",
            report.embedding.expected_dimension, report.embedding.actual_dimension
        )
    } else if let Some(rerank) = report.rerank {
        rerank
            .error
            .unwrap_or_else(|| "rerank provider preflight failed".to_string())
    } else {
        "knowledge provider preflight failed".to_string()
    };
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse { error: message }),
    )
}

fn not_found(message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: message.to_string(),
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

#[allow(dead_code)]
fn _assert_response_types_are_route_independent(
    _embedding: KnowledgeEmbeddingPreflight,
    _rerank: Option<KnowledgeRerankPreflight>,
    _status: KnowledgeUploadTaskStatus,
) {
}
