use std::sync::Arc;

use astrbot_kb::{
    ChunkId, ChunkingOptions, DocumentId, HybridKnowledgeRetriever,
    InMemoryKnowledgeBaseManagementStore, InMemoryKnowledgeDocumentRepository,
    InMemoryKnowledgeMediaStore, InMemoryKnowledgeUploadTaskStore, InMemoryVectorStore,
    KnowledgeBaseCatalog, KnowledgeBaseCreateCommand, KnowledgeBaseId,
    KnowledgeBaseManagementService, KnowledgeBaseSummary, KnowledgeBaseUpdateCommand,
    KnowledgeChunkCatalog, KnowledgeDocumentCatalog, KnowledgeDocumentRepository,
    KnowledgeDocumentSummary, KnowledgeEmbeddingPreflight, KnowledgeIngestionRequest,
    KnowledgeIngestionService, KnowledgeMediaStore, KnowledgeProviderPreflightReport,
    KnowledgeProviderPreflightRequest, KnowledgeProviderPreflightService, KnowledgeRerankPreflight,
    KnowledgeRetrievalRequest, KnowledgeRetriever, KnowledgeUploadProgress, KnowledgeUploadStage,
    KnowledgeUploadTaskId, KnowledgeUploadTaskKind, KnowledgeUploadTaskResult,
    KnowledgeUploadTaskService, KnowledgeUploadTaskStatus, KnowledgeUploadTaskSummary,
    PlainTextParser, RecursiveCharacterChunker, SqliteKnowledgeBaseManagementStore,
    SqliteKnowledgeDocumentRepository, SqliteVectorStore, VectorStore, VectorStorePersistencePort,
    VectorStoreSparseRetriever,
};
use astrbot_provider::{EmbeddingProvider, EmbeddingRequest, ProviderManager, RerankProvider};
use astrbot_storage::SqliteJsonStore;
use axum::{
    Json,
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone)]
pub struct ManagementKnowledgeBaseState {
    management: KnowledgeBaseManagementService,
    preflight: KnowledgeProviderPreflightService,
    upload_tasks: KnowledgeUploadTaskService,
    ingestion: Arc<KnowledgeIngestionService>,
    retriever: Arc<dyn KnowledgeRetriever>,
    vector_store: Arc<dyn VectorStore>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
    rerank_provider: Option<Arc<dyn RerankProvider>>,
}

impl std::fmt::Debug for ManagementKnowledgeBaseState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementKnowledgeBaseState")
            .finish_non_exhaustive()
    }
}

impl ManagementKnowledgeBaseState {
    pub fn new(
        management: KnowledgeBaseManagementService,
        preflight: KnowledgeProviderPreflightService,
        upload_tasks: KnowledgeUploadTaskService,
    ) -> Self {
        let provider_manager = Arc::new(ProviderManager::empty());
        Self::from_components(
            management,
            preflight,
            upload_tasks,
            provider_manager,
            None,
            Arc::new(InMemoryVectorStore::default()),
            Arc::new(InMemoryKnowledgeDocumentRepository::new()),
            Arc::new(InMemoryKnowledgeMediaStore::new()),
        )
    }

    pub fn in_memory(provider_manager: ProviderManager) -> Self {
        let provider_manager = Arc::new(provider_manager);
        Self::from_components(
            KnowledgeBaseManagementService::new(Arc::new(
                InMemoryKnowledgeBaseManagementStore::new(),
            )),
            KnowledgeProviderPreflightService::new(
                provider_manager.clone(),
                Some(provider_manager.clone()),
            ),
            KnowledgeUploadTaskService::new(Arc::new(InMemoryKnowledgeUploadTaskStore::new())),
            provider_manager.clone(),
            Some(provider_manager),
            Arc::new(InMemoryVectorStore::default()),
            Arc::new(InMemoryKnowledgeDocumentRepository::new()),
            Arc::new(InMemoryKnowledgeMediaStore::new()),
        )
    }

    pub fn sqlite(provider_manager: ProviderManager, store: SqliteJsonStore) -> Self {
        let provider_manager = Arc::new(provider_manager);
        Self::from_components(
            KnowledgeBaseManagementService::new(Arc::new(SqliteKnowledgeBaseManagementStore::new(
                store.clone(),
            ))),
            KnowledgeProviderPreflightService::new(
                provider_manager.clone(),
                Some(provider_manager.clone()),
            ),
            KnowledgeUploadTaskService::new(Arc::new(InMemoryKnowledgeUploadTaskStore::new())),
            provider_manager.clone(),
            Some(provider_manager),
            Arc::new(SqliteVectorStore::new(store.clone())),
            Arc::new(SqliteKnowledgeDocumentRepository::new(store)),
            Arc::new(InMemoryKnowledgeMediaStore::new()),
        )
    }

    pub fn from_components(
        management: KnowledgeBaseManagementService,
        preflight: KnowledgeProviderPreflightService,
        upload_tasks: KnowledgeUploadTaskService,
        embedding_provider: Arc<dyn EmbeddingProvider>,
        rerank_provider: Option<Arc<dyn RerankProvider>>,
        vector_store: Arc<dyn VectorStore>,
        repository: Arc<dyn KnowledgeDocumentRepository>,
        media_store: Arc<dyn KnowledgeMediaStore>,
    ) -> Self {
        let ingestion = Arc::new(KnowledgeIngestionService::new(
            Arc::new(PlainTextParser),
            Arc::new(RecursiveCharacterChunker::default()),
            embedding_provider.clone(),
            Arc::new(VectorStorePersistencePort::new(vector_store.clone())),
            repository,
            media_store,
        ));
        let retriever = Arc::new(HybridKnowledgeRetriever::new(
            vector_store.clone(),
            Arc::new(VectorStoreSparseRetriever::new(vector_store.clone())),
        ));
        Self {
            management,
            preflight,
            upload_tasks,
            ingestion,
            retriever,
            vector_store,
            embedding_provider,
            rerank_provider,
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

    pub fn vector_store(&self) -> Arc<dyn VectorStore> {
        self.vector_store.clone()
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k_dense: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k_sparse: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_m_final: Option<usize>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementKnowledgeRetrievalRequest {
    pub query: String,
    #[serde(default)]
    pub kb_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementKnowledgeIngestRequest {
    pub kb_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_id: Option<String>,
    pub name: String,
    #[serde(default = "default_source_kind")]
    pub source_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    pub content: String,
    #[serde(default)]
    pub clean_html: bool,
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementKnowledgeRetrievalHit {
    pub chunk_id: String,
    pub doc_id: String,
    pub kb_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kb_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_name: Option<String>,
    pub chunk_index: usize,
    pub content: String,
    pub score: f32,
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementKnowledgeRetrievalResponse {
    pub mode: String,
    pub query: String,
    pub results: Vec<ManagementKnowledgeRetrievalHit>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManagementKnowledgeIngestResponse {
    pub document: KnowledgeDocumentSummary,
    pub chunks: Vec<astrbot_kb::KnowledgeChunkSummary>,
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
        top_k_dense: request.top_k_dense,
        top_k_sparse: request.top_k_sparse,
        top_m_final: request.top_m_final,
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

pub async fn legacy_list(
    State(state): State<ManagementApiState>,
    Query(query): Query<std::collections::BTreeMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let _refresh_stats = query
        .get("refresh_stats")
        .map(|value| value == "true")
        .unwrap_or(false);
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let catalog = knowledge_base
        .management()
        .list_kbs()
        .await
        .map_err(internal_error)?;
    Ok(source_ok(json!({
        "items": catalog.knowledge_bases.into_iter().map(kb_to_source).collect::<Vec<_>>(),
    })))
}

pub async fn legacy_get(
    State(state): State<ManagementApiState>,
    Query(query): Query<std::collections::BTreeMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let kb_id = legacy_kb_id(&query).ok_or_else(|| bad_request_message("kb_id is required"))?;
    let kb_id = KnowledgeBaseId::new(kb_id).map_err(bad_request)?;
    let knowledge_base = knowledge_base
        .management()
        .get_kb(&kb_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("knowledge base not found"))?;
    Ok(source_ok(kb_to_source(knowledge_base)))
}

pub async fn legacy_create(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let name = string_field(&payload, "kb_name")
        .or_else(|| string_field(&payload, "name"))
        .ok_or_else(|| bad_request_message("kb_name is required"))?;
    let kb_id = string_field(&payload, "kb_id").unwrap_or_else(|| safe_id_fragment(&name));
    let embedding_provider_id = string_field(&payload, "embedding_provider_id")
        .ok_or_else(|| bad_request_message("embedding_provider_id is required"))?;
    let preflight = knowledge_base
        .preflight()
        .preflight(
            KnowledgeProviderPreflightRequest::new(&embedding_provider_id)
                .with_rerank_provider_id(string_field(&payload, "rerank_provider_id")),
        )
        .await
        .map_err(bad_request)?;
    if !preflight.is_usable() {
        return Err(preflight_failed(preflight));
    }
    let command = KnowledgeBaseCreateCommand::new(
        KnowledgeBaseId::new(kb_id).map_err(bad_request)?,
        name,
        embedding_provider_id,
    )
    .with_description(string_field(&payload, "description"))
    .with_emoji(string_field(&payload, "emoji"))
    .with_rerank_provider_id(string_field(&payload, "rerank_provider_id"))
    .with_chunking(
        number_field(&payload, "chunk_size"),
        number_field(&payload, "chunk_overlap"),
    )
    .with_retrieval_limits(
        number_field(&payload, "top_k_dense"),
        number_field(&payload, "top_k_sparse"),
        number_field(&payload, "top_m_final"),
    );
    let created = knowledge_base
        .management()
        .create_kb(command)
        .await
        .map_err(bad_request)?;
    Ok(source_ok(kb_to_source(created)))
}

pub async fn legacy_update(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let kb_id =
        string_field(&payload, "kb_id").ok_or_else(|| bad_request_message("kb_id is required"))?;
    let command = KnowledgeBaseUpdateCommand {
        name: string_field(&payload, "kb_name").or_else(|| string_field(&payload, "name")),
        description: optional_string_field(&payload, "description"),
        emoji: optional_string_field(&payload, "emoji"),
        embedding_provider_id: string_field(&payload, "embedding_provider_id"),
        rerank_provider_id: optional_string_field(&payload, "rerank_provider_id"),
        chunk_size: number_field(&payload, "chunk_size"),
        chunk_overlap: number_field(&payload, "chunk_overlap"),
        top_k_dense: number_field(&payload, "top_k_dense"),
        top_k_sparse: number_field(&payload, "top_k_sparse"),
        top_m_final: number_field(&payload, "top_m_final"),
    };
    let updated = knowledge_base
        .management()
        .update_kb(&KnowledgeBaseId::new(kb_id).map_err(bad_request)?, command)
        .await
        .map_err(bad_request)?
        .ok_or_else(|| not_found("knowledge base not found"))?;
    Ok(source_ok(kb_to_source(updated)))
}

pub async fn legacy_delete(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let kb_id =
        string_field(&payload, "kb_id").ok_or_else(|| bad_request_message("kb_id is required"))?;
    let response = delete(
        State(state),
        Json(ManagementKnowledgeBaseIdRequest {
            kb_id: kb_id.clone(),
        }),
    )
    .await?;
    Ok(source_ok(
        json!({ "deleted": response.0.ok, "kb_id": kb_id }),
    ))
}

pub async fn legacy_stats(
    State(state): State<ManagementApiState>,
    Query(query): Query<std::collections::BTreeMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let kb_id = legacy_kb_id(&query).ok_or_else(|| bad_request_message("kb_id is required"))?;
    let summary = knowledge_base
        .management()
        .get_kb(&KnowledgeBaseId::new(kb_id).map_err(bad_request)?)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("knowledge base not found"))?;
    Ok(source_ok(json!({
        "kb_id": summary.kb_id,
        "kb_name": summary.name,
        "doc_count": summary.stats.doc_count,
        "chunk_count": summary.stats.chunk_count,
        "created_at": Value::Null,
        "updated_at": Value::Null,
    })))
}

pub async fn legacy_document_list(
    State(state): State<ManagementApiState>,
    Query(query): Query<std::collections::BTreeMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let kb_id = legacy_kb_id(&query).ok_or_else(|| bad_request_message("kb_id is required"))?;
    let catalog = knowledge_base
        .management()
        .list_documents(&KnowledgeBaseId::new(kb_id).map_err(bad_request)?)
        .await
        .map_err(internal_error)?;
    Ok(source_ok(json!({
        "items": catalog.documents.into_iter().map(document_to_source).collect::<Vec<_>>(),
    })))
}

pub async fn legacy_document_get(
    State(state): State<ManagementApiState>,
    Query(query): Query<std::collections::BTreeMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let doc_id = string_field_map(&query, "doc_id")
        .ok_or_else(|| bad_request_message("doc_id is required"))?;
    let document = knowledge_base
        .management()
        .get_document(&DocumentId::new(doc_id).map_err(bad_request)?)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("knowledge document not found"))?;
    Ok(source_ok(document_to_source(document)))
}

pub async fn legacy_document_delete(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let doc_id = string_field(&payload, "doc_id")
        .ok_or_else(|| bad_request_message("doc_id is required"))?;
    let response = delete_document(
        State(state),
        Json(ManagementKnowledgeDocumentIdRequest {
            doc_id: doc_id.clone(),
        }),
    )
    .await?;
    Ok(source_ok(
        json!({ "deleted": response.0.ok, "doc_id": doc_id }),
    ))
}

pub async fn legacy_document_upload(
    State(state): State<ManagementApiState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let mut kb_id = String::new();
    let mut chunk_size = None;
    let mut chunk_overlap = None;
    let mut files = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(multipart_error)? {
        let name = field.name().unwrap_or_default().to_string();
        if name == "kb_id" {
            kb_id = field
                .text()
                .await
                .map_err(multipart_error)?
                .trim()
                .to_string();
            continue;
        }
        if name == "chunk_size" {
            chunk_size = field
                .text()
                .await
                .map_err(multipart_error)?
                .trim()
                .parse::<usize>()
                .ok();
            continue;
        }
        if name == "chunk_overlap" {
            chunk_overlap = field
                .text()
                .await
                .map_err(multipart_error)?
                .trim()
                .parse::<usize>()
                .ok();
            continue;
        }
        if name == "file" || name.starts_with("file") || name == "files[]" {
            let filename = field.file_name().unwrap_or("upload.txt").to_string();
            let bytes = field.bytes().await.map_err(multipart_error)?.to_vec();
            files.push((filename, bytes));
        }
    }

    if kb_id.is_empty() {
        return Err(bad_request_message("kb_id is required"));
    }
    if files.is_empty() {
        return Err(bad_request_message("multipart file is required"));
    }
    if files.len() > 10 {
        return Err(bad_request_message("at most 10 files can be uploaded"));
    }

    let task_id = format!(
        "upload-{}",
        safe_id_fragment(&format!("{}-{}", kb_id, files.len()))
    );
    let task = start_upload_task(
        &state,
        &task_id,
        KnowledgeUploadTaskKind::Upload,
        &kb_id,
        files.len(),
    )
    .await?;
    let mut document_ids = Vec::new();
    let mut chunk_count = 0usize;
    for (index, (filename, bytes)) in files.into_iter().enumerate() {
        update_upload_stage(
            &state,
            &task_id,
            index,
            filename.clone(),
            KnowledgeUploadStage::Parsing,
            0,
            1,
        )
        .await?;
        let doc_id = format!("doc-{}", safe_id_fragment(&filename));
        let extension = filename
            .rsplit_once('.')
            .map(|(_, extension)| extension)
            .unwrap_or("txt")
            .to_string();
        let ingest_response = ingest(
            State(state.clone()),
            Json(ManagementKnowledgeIngestRequest {
                kb_id: kb_id.clone(),
                doc_id: Some(doc_id.clone()),
                name: filename.clone(),
                source_kind: extension,
                source_url: None,
                content: String::from_utf8_lossy(&bytes).into_owned(),
                clean_html: false,
            }),
        )
        .await?;
        let _ = (chunk_size, chunk_overlap);
        document_ids.push(ingest_response.0.document.doc_id);
        chunk_count += ingest_response.0.chunks.len();
        update_upload_stage(
            &state,
            &task_id,
            index,
            filename,
            KnowledgeUploadStage::Embedding,
            ingest_response.0.chunks.len(),
            ingest_response.0.chunks.len().max(1),
        )
        .await?;
    }
    complete_upload_task(&state, &task_id, document_ids.clone(), chunk_count).await?;
    Ok(source_ok(json!({
        "task_id": task.task_id,
        "file_count": document_ids.len(),
        "message": "task created, processing in background",
    })))
}

pub async fn legacy_document_import(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let kb_id =
        string_field(&payload, "kb_id").ok_or_else(|| bad_request_message("kb_id is required"))?;
    let documents = payload
        .get("documents")
        .and_then(Value::as_array)
        .ok_or_else(|| bad_request_message("documents is required"))?;
    if documents.is_empty() {
        return Err(bad_request_message("documents is required"));
    }
    let task_id = format!("import-{}", safe_id_fragment(&kb_id));
    start_upload_task(
        &state,
        &task_id,
        KnowledgeUploadTaskKind::Import,
        &kb_id,
        documents.len(),
    )
    .await?;
    let mut document_ids = Vec::new();
    let mut chunk_count = 0usize;
    for (index, document) in documents.iter().enumerate() {
        let file_name = string_field(document, "file_name")
            .or_else(|| string_field(document, "name"))
            .ok_or_else(|| bad_request_message("document file_name is required"))?;
        let chunks = document
            .get("chunks")
            .and_then(Value::as_array)
            .ok_or_else(|| bad_request_message("document chunks is required"))?;
        let content = chunks
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("\n\n");
        if content.trim().is_empty() {
            return Err(bad_request_message("document chunks is required"));
        }
        update_upload_stage(
            &state,
            &task_id,
            index,
            file_name.clone(),
            KnowledgeUploadStage::Parsing,
            0,
            chunks.len().max(1),
        )
        .await?;
        let doc_id = string_field(document, "doc_id")
            .unwrap_or_else(|| format!("doc-{}", safe_id_fragment(&file_name)));
        let ingest_response = ingest(
            State(state.clone()),
            Json(ManagementKnowledgeIngestRequest {
                kb_id: kb_id.clone(),
                doc_id: Some(doc_id),
                name: file_name.clone(),
                source_kind: string_field(document, "file_type")
                    .unwrap_or_else(|| "import".to_string()),
                source_url: None,
                content,
                clean_html: false,
            }),
        )
        .await?;
        document_ids.push(ingest_response.0.document.doc_id);
        chunk_count += ingest_response.0.chunks.len();
        update_upload_stage(
            &state,
            &task_id,
            index,
            file_name,
            KnowledgeUploadStage::Completed,
            chunks.len(),
            chunks.len().max(1),
        )
        .await?;
    }
    complete_upload_task(&state, &task_id, document_ids.clone(), chunk_count).await?;
    Ok(source_ok(json!({
        "task_id": task_id,
        "doc_count": document_ids.len(),
        "message": "import task created, processing in background",
    })))
}

pub async fn legacy_chunk_list(
    State(state): State<ManagementApiState>,
    Query(query): Query<std::collections::BTreeMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let doc_id = string_field_map(&query, "doc_id")
        .ok_or_else(|| bad_request_message("doc_id is required"))?;
    let page = query
        .get("page")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let page_size = query
        .get("page_size")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(25)
        .clamp(1, 200);
    let catalog = knowledge_base
        .management()
        .list_chunks_for_document(&DocumentId::new(doc_id).map_err(bad_request)?)
        .await
        .map_err(internal_error)?;
    let total = catalog.chunks.len();
    let start = (page - 1) * page_size;
    let items = catalog
        .chunks
        .into_iter()
        .skip(start)
        .take(page_size)
        .map(chunk_to_source)
        .collect::<Vec<_>>();
    Ok(source_ok(
        json!({ "items": items, "total": total, "page": page, "page_size": page_size }),
    ))
}

pub async fn legacy_chunk_delete(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let chunk_id = string_field(&payload, "chunk_id")
        .ok_or_else(|| bad_request_message("chunk_id is required"))?;
    let response = delete_chunk(
        State(state),
        Json(ManagementKnowledgeChunkDeleteRequest {
            chunk_id: chunk_id.clone(),
        }),
    )
    .await?;
    Ok(source_ok(
        json!({ "deleted": response.0.ok, "chunk_id": chunk_id }),
    ))
}

pub async fn legacy_retrieve(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let query =
        string_field(&payload, "query").ok_or_else(|| bad_request_message("query is required"))?;
    let mut kb_ids = string_vec_field(&payload, "kb_ids");
    if kb_ids.is_empty() {
        kb_ids = string_vec_field(&payload, "kb_names");
    }
    let response = retrieve(
        State(state),
        Json(ManagementKnowledgeRetrievalRequest {
            query,
            kb_ids,
            top_k: number_field(&payload, "top_k"),
        }),
    )
    .await?;
    Ok(source_ok(json!({
        "mode": response.0.mode,
        "query": response.0.query,
        "results": response.0.results.into_iter().map(retrieval_hit_to_source).collect::<Vec<_>>(),
        "visualization": Value::Null,
    })))
}

pub async fn legacy_document_upload_url(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let kb_id =
        string_field(&payload, "kb_id").ok_or_else(|| bad_request_message("kb_id is required"))?;
    let url =
        string_field(&payload, "url").ok_or_else(|| bad_request_message("url is required"))?;
    let task_id = format!("url-{}", safe_id_fragment(&url));
    start_upload_task(&state, &task_id, KnowledgeUploadTaskKind::Url, &kb_id, 1).await?;
    update_upload_stage(
        &state,
        &task_id,
        0,
        url.clone(),
        KnowledgeUploadStage::Extracting,
        0,
        1,
    )
    .await?;
    let content = payload
        .get("content")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("Imported URL: {url}"));
    let doc_id = string_field(&payload, "doc_id")
        .or_else(|| Some(format!("doc-{}", safe_id_fragment(&url))));
    let response = ingest(
        State(state.clone()),
        Json(ManagementKnowledgeIngestRequest {
            kb_id: kb_id.clone(),
            doc_id: doc_id.clone(),
            name: url.clone(),
            source_kind: "url".to_string(),
            source_url: Some(url.clone()),
            content,
            clean_html: bool_field(&payload, "enable_cleaning"),
        }),
    )
    .await?;
    complete_upload_task(
        &state,
        &task_id,
        vec![response.0.document.doc_id],
        response.0.chunks.len(),
    )
    .await?;
    Ok(source_ok(
        json!({ "task_id": task_id, "url": url, "file_count": 1 }),
    ))
}

pub async fn legacy_upload_progress(
    State(state): State<ManagementApiState>,
    Query(query): Query<std::collections::BTreeMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let task_id = string_field_map(&query, "task_id")
        .ok_or_else(|| bad_request_message("task_id is required"))?;
    let response = upload_progress(State(state), Path(task_id)).await?;
    Ok(source_ok(upload_task_to_source(response.0.task)))
}

pub async fn delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeBaseIdRequest>,
) -> Result<Json<ManagementKnowledgeMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let kb_id = KnowledgeBaseId::new(request.kb_id).map_err(bad_request)?;
    let documents = knowledge_base
        .management()
        .list_documents(&kb_id)
        .await
        .map_err(internal_error)?
        .documents;
    let deleted = knowledge_base
        .management()
        .delete_kb(&kb_id)
        .await
        .map_err(internal_error)?;
    if !deleted {
        return Err(not_found("knowledge base not found"));
    }
    for document in documents {
        let doc_id = DocumentId::new(document.doc_id).map_err(bad_request)?;
        knowledge_base
            .vector_store()
            .delete_document(&doc_id)
            .await
            .map_err(internal_error)?;
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
    knowledge_base
        .vector_store()
        .delete_document(&doc_id)
        .await
        .map_err(internal_error)?;
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
    knowledge_base
        .vector_store()
        .delete_chunk(&chunk_id)
        .await
        .map_err(internal_error)?;
    Ok(Json(ManagementKnowledgeMutationResponse { ok: true }))
}

pub async fn retrieve(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeRetrievalRequest>,
) -> Result<Json<ManagementKnowledgeRetrievalResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let query = request.query.trim().to_string();
    if query.is_empty() {
        return Err(bad_request_message("query is required"));
    }

    let kb_ids = if request.kb_ids.is_empty() {
        knowledge_base
            .management()
            .list_kbs()
            .await
            .map_err(internal_error)?
            .knowledge_bases
            .into_iter()
            .map(|summary| KnowledgeBaseId::new(summary.kb_id).map_err(bad_request))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        request
            .kb_ids
            .into_iter()
            .map(KnowledgeBaseId::new)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(bad_request)?
    };
    if kb_ids.is_empty() {
        return Ok(Json(ManagementKnowledgeRetrievalResponse {
            mode: "hybrid_vector".to_string(),
            query,
            results: Vec::new(),
        }));
    }

    let mut summaries = Vec::new();
    for kb_id in &kb_ids {
        let Some(summary) = knowledge_base
            .management()
            .get_kb(kb_id)
            .await
            .map_err(internal_error)?
        else {
            return Err(not_found("knowledge base not found"));
        };
        summaries.push(summary);
    }

    let embedding_provider_id = summaries
        .first()
        .map(|summary| summary.embedding_provider_id.clone());
    let mut embedding_request = EmbeddingRequest::new(query.clone());
    if let Some(provider_id) = embedding_provider_id {
        embedding_request = embedding_request.with_provider_id(provider_id);
    }
    let query_embedding = knowledge_base
        .embedding_provider
        .embed(embedding_request)
        .await
        .map_err(internal_error)?
        .embeddings
        .into_iter()
        .next()
        .ok_or_else(|| {
            internal_error(astrbot_kb::kb_error(
                "embedding provider returned no vector",
            ))
        })?;
    let top_m_final = request
        .top_k
        .unwrap_or_else(|| {
            summaries
                .iter()
                .map(|summary| summary.top_m_final)
                .max()
                .unwrap_or(5)
        })
        .clamp(1, 50);
    let top_k_dense = summaries
        .iter()
        .map(|summary| summary.top_k_dense)
        .max()
        .unwrap_or(50)
        .clamp(1, 100);
    let top_k_sparse = summaries
        .iter()
        .map(|summary| summary.top_k_sparse)
        .max()
        .unwrap_or(50)
        .clamp(1, 100);
    let rerank_provider_id = summaries
        .iter()
        .find_map(|summary| summary.rerank_provider_id.clone());
    let results = knowledge_base
        .retriever
        .retrieve(
            KnowledgeRetrievalRequest::new(query.clone(), kb_ids)
                .with_query_embedding(query_embedding)
                .with_rerank_provider_id(rerank_provider_id.clone())
                .with_limits(
                    top_k_dense,
                    top_k_sparse,
                    top_k_dense + top_k_sparse,
                    top_m_final,
                ),
            rerank_provider_id
                .as_ref()
                .and_then(|_| knowledge_base.rerank_provider.clone()),
        )
        .await
        .map_err(internal_error)?
        .into_iter()
        .map(|result| ManagementKnowledgeRetrievalHit {
            chunk_id: result.chunk_id,
            doc_id: result.doc_id,
            kb_id: result.kb_id,
            kb_name: result.kb_name,
            doc_name: result.doc_name,
            chunk_index: result.chunk_index,
            content: result.content,
            score: result.score,
            metadata: result.metadata,
        })
        .collect();

    Ok(Json(ManagementKnowledgeRetrievalResponse {
        mode: "hybrid_vector".to_string(),
        query,
        results,
    }))
}

pub async fn ingest(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementKnowledgeIngestRequest>,
) -> Result<Json<ManagementKnowledgeIngestResponse>, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let kb_id = KnowledgeBaseId::new(request.kb_id).map_err(bad_request)?;
    let kb = knowledge_base
        .management()
        .get_kb(&kb_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("knowledge base not found"))?;
    let content = if request.clean_html {
        strip_html_tags(&request.content)
    } else {
        request.content
    };
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err(bad_request_message("content is required"));
    }
    let name = request.name.trim();
    if name.is_empty() {
        return Err(bad_request_message("document name is required"));
    }
    let doc_id = match request.doc_id {
        Some(doc_id) => DocumentId::new(doc_id).map_err(bad_request)?,
        None => DocumentId::new(format!("doc-{}", safe_id_fragment(name))).map_err(bad_request)?,
    };
    for chunk in knowledge_base
        .management()
        .list_chunks_for_document(&doc_id)
        .await
        .map_err(internal_error)?
        .chunks
    {
        let chunk_id = ChunkId::new(chunk.chunk_id).map_err(bad_request)?;
        knowledge_base
            .management()
            .delete_chunk(&chunk_id)
            .await
            .map_err(internal_error)?;
    }
    knowledge_base
        .vector_store()
        .delete_document(&doc_id)
        .await
        .map_err(internal_error)?;

    let chunking = ChunkingOptions::new(kb.chunk_size, kb.chunk_overlap).map_err(bad_request)?;
    let mut ingestion_request = KnowledgeIngestionRequest::new(
        kb_id.clone(),
        doc_id.clone(),
        name.to_string(),
        request.source_kind.trim(),
        content.into_bytes(),
    )
    .with_embedding_provider_id(kb.embedding_provider_id.clone());
    ingestion_request.chunking = chunking;
    let mut outcome = knowledge_base
        .ingestion
        .ingest(ingestion_request)
        .await
        .map_err(internal_error)?;
    outcome.document.file_path = request.source_url.clone();
    knowledge_base
        .management()
        .upsert_document(outcome.document.clone())
        .await
        .map_err(internal_error)?;

    let mut summaries = Vec::with_capacity(outcome.chunks.len());
    for mut chunk in outcome.chunks {
        chunk = chunk.with_metadata("source_kind", serde_json::json!(request.source_kind));
        if let Some(source_url) = &request.source_url {
            chunk = chunk.with_metadata("source_url", serde_json::json!(source_url));
        }
        if request.clean_html {
            chunk = chunk.with_metadata("clean_html", serde_json::json!(true));
        }
        knowledge_base
            .management()
            .upsert_chunk(chunk.clone())
            .await
            .map_err(internal_error)?;
        summaries.push(chunk.into());
    }

    Ok(Json(ManagementKnowledgeIngestResponse {
        document: outcome.document.into(),
        chunks: summaries,
    }))
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

async fn start_upload_task(
    state: &ManagementApiState,
    task_id: &str,
    kind: KnowledgeUploadTaskKind,
    kb_id: &str,
    file_total: usize,
) -> Result<KnowledgeUploadTaskSummary, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    knowledge_base
        .upload_tasks()
        .start_task(
            KnowledgeUploadTaskId::new(task_id).map_err(bad_request)?,
            kind,
            kb_id.to_string(),
            file_total,
        )
        .await
        .map_err(internal_error)
}

async fn update_upload_stage(
    state: &ManagementApiState,
    task_id: &str,
    file_index: usize,
    file_name: String,
    stage: KnowledgeUploadStage,
    current: usize,
    total: usize,
) -> Result<KnowledgeUploadTaskSummary, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let task_id = KnowledgeUploadTaskId::new(task_id).map_err(bad_request)?;
    let progress = KnowledgeUploadProgress::queued(total.max(1)).processing(
        file_index,
        file_name,
        stage,
        current,
        total.max(1),
    );
    knowledge_base
        .upload_tasks()
        .update_progress(&task_id, progress)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("knowledge upload task not found"))
}

async fn complete_upload_task(
    state: &ManagementApiState,
    task_id: &str,
    document_ids: Vec<String>,
    chunk_count: usize,
) -> Result<KnowledgeUploadTaskSummary, (StatusCode, Json<ErrorResponse>)> {
    let knowledge_base = state
        .knowledge_base()
        .ok_or_else(knowledge_base_unavailable)?;
    let task_id = KnowledgeUploadTaskId::new(task_id).map_err(bad_request)?;
    knowledge_base
        .upload_tasks()
        .complete_task(
            &task_id,
            KnowledgeUploadTaskResult::new(document_ids, chunk_count),
        )
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("knowledge upload task not found"))
}

fn default_file_total() -> usize {
    1
}

fn default_source_kind() -> String {
    "text".to_string()
}

fn strip_html_tags(content: &str) -> String {
    let mut output = String::with_capacity(content.len());
    let mut in_tag = false;
    for ch in content.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output
}

fn safe_id_fragment(value: &str) -> String {
    let fragment = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if fragment.is_empty() {
        "document".to_string()
    } else {
        fragment
    }
}

fn kb_to_source(kb: KnowledgeBaseSummary) -> Value {
    json!({
        "kb_id": kb.kb_id,
        "kb_name": kb.name,
        "name": kb.name,
        "description": kb.description,
        "emoji": kb.emoji,
        "embedding_provider_id": kb.embedding_provider_id,
        "rerank_provider_id": kb.rerank_provider_id,
        "chunk_size": kb.chunk_size,
        "chunk_overlap": kb.chunk_overlap,
        "top_k_dense": kb.top_k_dense,
        "top_k_sparse": kb.top_k_sparse,
        "top_m_final": kb.top_m_final,
        "doc_count": kb.stats.doc_count,
        "chunk_count": kb.stats.chunk_count,
        "stats": kb.stats,
        "created_at": Value::Null,
        "updated_at": Value::Null,
    })
}

fn document_to_source(document: KnowledgeDocumentSummary) -> Value {
    json!({
        "doc_id": document.doc_id,
        "kb_id": document.kb_id,
        "doc_name": document.name,
        "name": document.name,
        "file_type": document.file_type,
        "file_size": document.file_size,
        "file_path": document.file_path,
        "chunk_count": document.chunk_count,
        "media_count": document.media_count,
        "created_at": Value::Null,
        "updated_at": Value::Null,
    })
}

fn chunk_to_source(chunk: astrbot_kb::KnowledgeChunkSummary) -> Value {
    json!({
        "chunk_id": chunk.chunk_id,
        "doc_id": chunk.doc_id,
        "kb_id": chunk.kb_id,
        "chunk_index": chunk.chunk_index,
        "content": chunk.content,
        "char_count": chunk.char_count,
        "metadata": chunk.metadata,
    })
}

fn retrieval_hit_to_source(hit: ManagementKnowledgeRetrievalHit) -> Value {
    let char_count = hit.content.chars().count();
    json!({
        "chunk_id": hit.chunk_id,
        "doc_id": hit.doc_id,
        "kb_id": hit.kb_id,
        "kb_name": hit.kb_name,
        "doc_name": hit.doc_name,
        "chunk_index": hit.chunk_index,
        "content": hit.content,
        "char_count": char_count,
        "score": hit.score,
        "metadata": hit.metadata,
    })
}

fn upload_task_to_source(task: KnowledgeUploadTaskSummary) -> Value {
    json!({
        "task_id": task.task_id,
        "kb_id": task.kb_id,
        "kind": task.kind,
        "status": task.status,
        "file_total": task.progress.as_ref().map(|progress| progress.file_total).unwrap_or(0),
        "progress": task.progress,
        "result": task.result,
        "error": task.error,
    })
}

fn legacy_kb_id(query: &std::collections::BTreeMap<String, String>) -> Option<String> {
    string_field_map(query, "kb_id").or_else(|| string_field_map(query, "kb_name"))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn optional_string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|raw| {
        if raw.is_null() {
            Some(String::new())
        } else {
            raw.as_str().map(ToOwned::to_owned)
        }
    })
}

fn string_field_map(map: &std::collections::BTreeMap<String, String>, key: &str) -> Option<String> {
    map.get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn number_field(value: &Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(|raw| raw.as_u64().or_else(|| raw.as_str()?.parse::<u64>().ok()))
        .map(|number| number as usize)
}

fn bool_field(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(|raw| {
            raw.as_bool()
                .or_else(|| raw.as_str().map(|text| text == "true"))
        })
        .unwrap_or(false)
}

fn string_vec_field(value: &Value, key: &str) -> Vec<String> {
    match value.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        Some(Value::String(text)) => text
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({ "status": "ok", "message": "", "data": data }))
}

fn multipart_error(
    error: axum::extract::multipart::MultipartError,
) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: format!("knowledge base multipart upload: {error}"),
        }),
    )
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

fn bad_request_message(message: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: message.to_string(),
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
