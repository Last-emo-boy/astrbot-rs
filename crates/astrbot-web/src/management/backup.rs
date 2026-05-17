use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use astrbot_storage::{
    BACKUP_UPLOAD_CHUNK_SIZE, BackupChunkReceipt, BackupExportJobRequest, BackupImportJobRequest,
    BackupImportMode, BackupImportPrecheck, BackupJobService, BackupJobSnapshot, BackupManifest,
    BackupProgressReader, BackupUploadCompletePlan, BackupUploadManager, BackupUploadSession,
    BackupUploadStart,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone)]
pub struct ManagementBackupState {
    service: Arc<BackupJobService>,
    uploads: Arc<Mutex<BackupUploadManager>>,
    chunk_root: PathBuf,
}

impl ManagementBackupState {
    pub fn new(service: Arc<BackupJobService>, chunk_root: impl Into<PathBuf>) -> Self {
        Self {
            service,
            uploads: Arc::new(Mutex::new(BackupUploadManager::new())),
            chunk_root: chunk_root.into(),
        }
    }

    pub fn with_uploads(
        service: Arc<BackupJobService>,
        uploads: BackupUploadManager,
        chunk_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            service,
            uploads: Arc::new(Mutex::new(uploads)),
            chunk_root: chunk_root.into(),
        }
    }

    pub fn service(&self) -> Arc<BackupJobService> {
        self.service.clone()
    }

    pub fn chunk_root(&self) -> &PathBuf {
        &self.chunk_root
    }
}

impl fmt::Debug for ManagementBackupState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagementBackupState")
            .field("chunk_root", &self.chunk_root)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupPrecheckRequest {
    pub manifest: BackupManifest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupPrecheckResponse {
    pub precheck: BackupImportPrecheck,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupExportRequest {
    pub task_id: String,
    pub astrbot_version: String,
    pub exported_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupImportRequest {
    pub task_id: String,
    pub source_id: String,
    pub mode: BackupImportMode,
    pub confirmed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupJobResponse {
    pub task: BackupJobSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupProgressResponse {
    pub task: BackupJobSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupUploadStartRequest {
    pub upload_id: String,
    pub filename: String,
    pub total_size: u64,
    pub now_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupUploadStartResponse {
    pub session: BackupUploadSession,
    pub chunk_size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupChunkRequest {
    pub upload_id: String,
    pub chunk_index: u32,
    pub bytes_len: u64,
    pub now_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupChunkResponse {
    pub receipt: BackupChunkReceipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupCompleteRequest {
    pub upload_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupCompleteResponse {
    pub plan: BackupUploadCompletePlan,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupAbortRequest {
    pub upload_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupAbortResponse {
    pub aborted: bool,
}

pub async fn precheck(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupPrecheckRequest>,
) -> Result<Json<ManagementBackupPrecheckResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let precheck = backup
        .service()
        .precheck_manifest(&request.manifest)
        .await
        .map_err(map_backup_error)?;

    Ok(Json(ManagementBackupPrecheckResponse { precheck }))
}

pub async fn export(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupExportRequest>,
) -> Result<Json<ManagementBackupJobResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let task = backup
        .service()
        .start_export(BackupExportJobRequest::new(
            request.task_id,
            request.astrbot_version,
            request.exported_at,
        ))
        .await
        .map_err(map_backup_error)?;

    Ok(Json(ManagementBackupJobResponse { task }))
}

pub async fn import(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupImportRequest>,
) -> Result<Json<ManagementBackupJobResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !request.confirmed {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "backup import must be confirmed".to_string(),
            }),
        ));
    }

    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let task = backup
        .service()
        .start_import(BackupImportJobRequest::new(
            request.task_id,
            request.source_id,
            request.mode,
        ))
        .await
        .map_err(map_backup_error)?;

    Ok(Json(ManagementBackupJobResponse { task }))
}

pub async fn progress(
    State(state): State<ManagementApiState>,
    Path(task_id): Path<String>,
) -> Result<Json<ManagementBackupProgressResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let task = backup
        .service()
        .progress_snapshot(&task_id)
        .await
        .map_err(map_backup_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("backup task {task_id} was not found"),
                }),
            )
        })?;

    Ok(Json(ManagementBackupProgressResponse { task }))
}

pub async fn upload_start(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupUploadStartRequest>,
) -> Result<Json<ManagementBackupUploadStartResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let chunk_dir = backup.chunk_root().join(request.upload_id.trim());
    let total_chunks = total_chunks(request.total_size)?;
    let session = backup
        .uploads
        .lock()
        .map_err(upload_lock_error)?
        .start_upload(
            BackupUploadStart {
                upload_id: request.upload_id,
                original_filename: request.filename,
                total_size: request.total_size,
                total_chunks,
                chunk_dir,
            },
            request.now_unix,
        )
        .map_err(map_backup_error)?;

    Ok(Json(ManagementBackupUploadStartResponse {
        session,
        chunk_size: BACKUP_UPLOAD_CHUNK_SIZE,
    }))
}

pub async fn upload_chunk(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupChunkRequest>,
) -> Result<Json<ManagementBackupChunkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let receipt = backup
        .uploads
        .lock()
        .map_err(upload_lock_error)?
        .receive_chunk(
            &request.upload_id,
            request.chunk_index,
            request.bytes_len,
            request.now_unix,
        )
        .map_err(map_backup_error)?;

    Ok(Json(ManagementBackupChunkResponse { receipt }))
}

pub async fn upload_complete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupCompleteRequest>,
) -> Result<Json<ManagementBackupCompleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let mut uploads = backup.uploads.lock().map_err(upload_lock_error)?;
    let plan = uploads
        .complete_plan(&request.upload_id)
        .map_err(map_backup_error)?;
    let _ = uploads.abort(&request.upload_id);

    Ok(Json(ManagementBackupCompleteResponse { plan }))
}

pub async fn upload_abort(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupAbortRequest>,
) -> Result<Json<ManagementBackupAbortResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let aborted = backup
        .uploads
        .lock()
        .map_err(upload_lock_error)?
        .abort(&request.upload_id)
        .is_some();

    Ok(Json(ManagementBackupAbortResponse { aborted }))
}

fn backup_state_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "management backup state is not configured".to_string(),
        }),
    )
}

fn map_backup_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn total_chunks(total_size: u64) -> Result<u32, (StatusCode, Json<ErrorResponse>)> {
    if total_size == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "backup upload total_size must be greater than zero".to_string(),
            }),
        ));
    }
    let chunks = total_size.div_ceil(BACKUP_UPLOAD_CHUNK_SIZE);
    u32::try_from(chunks).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "backup upload has too many chunks".to_string(),
            }),
        )
    })
}

fn upload_lock_error(
    error: std::sync::PoisonError<std::sync::MutexGuard<'_, BackupUploadManager>>,
) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: format!("backup upload session lock: {error}"),
        }),
    )
}
