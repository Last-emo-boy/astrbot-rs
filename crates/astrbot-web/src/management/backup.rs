use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_storage::{
    BACKUP_UPLOAD_CHUNK_SIZE, BackupChunkReceipt, BackupExportJobRequest, BackupImportJobRequest,
    BackupImportMode, BackupImportPrecheck, BackupJobKind, BackupJobService, BackupJobSnapshot,
    BackupJobStatus, BackupManifest, BackupProgressReader, BackupProgressSnapshot,
    BackupUploadCompletePlan, BackupUploadManager, BackupUploadSession, BackupUploadStart,
    FileTokenRecord, FileTokenScope, merge_upload_chunks,
};
use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone)]
pub struct ManagementBackupState {
    service: Arc<BackupJobService>,
    uploads: Arc<Mutex<BackupUploadManager>>,
    backup_root: PathBuf,
    chunk_root: PathBuf,
}

impl ManagementBackupState {
    pub fn new(service: Arc<BackupJobService>, chunk_root: impl Into<PathBuf>) -> Self {
        let chunk_root = chunk_root.into();
        Self {
            service,
            uploads: Arc::new(Mutex::new(BackupUploadManager::new())),
            backup_root: chunk_root.clone(),
            chunk_root,
        }
    }

    pub fn with_roots(
        service: Arc<BackupJobService>,
        backup_root: impl Into<PathBuf>,
        chunk_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            service,
            uploads: Arc::new(Mutex::new(BackupUploadManager::new())),
            backup_root: backup_root.into(),
            chunk_root: chunk_root.into(),
        }
    }

    pub fn with_uploads(
        service: Arc<BackupJobService>,
        uploads: BackupUploadManager,
        chunk_root: impl Into<PathBuf>,
    ) -> Self {
        let chunk_root = chunk_root.into();
        Self {
            service,
            uploads: Arc::new(Mutex::new(uploads)),
            backup_root: chunk_root.clone(),
            chunk_root,
        }
    }

    pub fn service(&self) -> Arc<BackupJobService> {
        self.service.clone()
    }

    pub fn chunk_root(&self) -> &PathBuf {
        &self.chunk_root
    }

    pub fn backup_root(&self) -> &PathBuf {
        &self.backup_root
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
    pub manifest: Option<BackupManifest>,
    pub filename: Option<String>,
    pub source_id: Option<String>,
    pub task_id: Option<String>,
    pub mode: Option<BackupImportMode>,
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
pub struct ManagementBackupProgressCatalogResponse {
    pub tasks: Vec<BackupJobSnapshot>,
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
    pub bytes_base64: Option<String>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupFileInfo {
    pub filename: String,
    pub size_bytes: u64,
    pub modified_at_unix: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupFileCatalogResponse {
    pub files: Vec<ManagementBackupFileInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupFileRequest {
    pub filename: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupFileDownloadResponse {
    pub filename: String,
    pub token: String,
    pub download_url: String,
    pub expires_at_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupFileRenameRequest {
    pub filename: String,
    pub new_filename: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupFileRenameResponse {
    pub file: ManagementBackupFileInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupFileDeleteResponse {
    pub deleted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementBackupFileRestoreRequest {
    pub filename: String,
    pub task_id: String,
    pub mode: BackupImportMode,
    pub confirmed: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyBackupListQuery {
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyBackupProgressQuery {
    pub task_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyBackupDownloadQuery {
    pub filename: String,
    #[serde(default)]
    pub token: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyBackupUploadInitRequest {
    pub filename: String,
    pub total_size: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyBackupCheckRequest {
    pub filename: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyBackupImportRequest {
    pub filename: String,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub task_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyBackupRenameRequest {
    pub filename: String,
    pub new_name: String,
}

pub async fn precheck(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupPrecheckRequest>,
) -> Result<Json<ManagementBackupPrecheckResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let precheck = if let Some(manifest) = request.manifest {
        backup
            .service()
            .precheck_manifest(&manifest)
            .await
            .map_err(map_backup_error)?
    } else {
        let source_id = request
            .source_id
            .or(request.filename)
            .ok_or_else(|| backup_bad_request("backup precheck requires filename".to_string()))?;
        let filename = validated_backup_filename(&source_id)?;
        let path = backup_file_path(backup.backup_root(), &filename)?;
        ensure_backup_file_exists(&path)?;
        backup
            .service()
            .precheck_import(&BackupImportJobRequest::new(
                request.task_id.unwrap_or_else(|| "precheck".to_string()),
                filename,
                request.mode.unwrap_or(BackupImportMode::Merge),
            ))
            .await
            .map_err(map_backup_error)?
    };

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
    let source_id = validated_backup_filename(&request.source_id)?;
    let path = backup_file_path(backup.backup_root(), &source_id)?;
    ensure_backup_file_exists(&path)?;
    let task = backup
        .service()
        .start_import(BackupImportJobRequest::new(
            request.task_id,
            source_id,
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

pub async fn progress_catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementBackupProgressCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let tasks = backup
        .service()
        .progress_snapshots()
        .await
        .map_err(map_backup_error)?;

    Ok(Json(ManagementBackupProgressCatalogResponse { tasks }))
}

pub async fn upload_start(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupUploadStartRequest>,
) -> Result<Json<ManagementBackupUploadStartResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let chunk_dir = backup.chunk_root().join(request.upload_id.trim());
    fs::create_dir_all(&chunk_dir).map_err(map_io_backup_error)?;
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
    backup
        .service()
        .jobs()
        .create(
            &session.upload_id,
            BackupJobKind::Upload,
            BackupProgressSnapshot::running(
                "upload",
                0,
                u64::from(session.total_chunks),
                "backup upload started",
            ),
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
    let chunk_bytes = decode_chunk_bytes(&request)?;
    let bytes_len = u64::try_from(chunk_bytes.len()).map_err(|_| {
        backup_bad_request("backup upload chunk is too large to measure".to_string())
    })?;
    if bytes_len != request.bytes_len {
        return Err(backup_bad_request(format!(
            "backup upload chunk length mismatch: declared {}, actual {bytes_len}",
            request.bytes_len
        )));
    }
    let receipt = backup
        .uploads
        .lock()
        .map_err(upload_lock_error)?
        .receive_chunk(
            &request.upload_id,
            request.chunk_index,
            bytes_len,
            request.now_unix,
        )
        .map_err(map_backup_error)?;
    let chunk_path = backup
        .uploads
        .lock()
        .map_err(upload_lock_error)?
        .session(&request.upload_id)
        .map(|session| {
            session
                .chunk_dir
                .join(format!("{}.part", request.chunk_index))
        })
        .ok_or_else(|| backup_bad_request("backup upload session was not found".to_string()))?;
    if let Some(parent) = chunk_path.parent() {
        fs::create_dir_all(parent).map_err(map_io_backup_error)?;
    }
    fs::write(chunk_path, chunk_bytes).map_err(map_io_backup_error)?;
    backup
        .service()
        .jobs()
        .update_progress(
            &request.upload_id,
            BackupProgressSnapshot::running(
                "upload",
                u64::from(receipt.received_chunks),
                u64::from(receipt.total_chunks),
                "backup upload chunk received",
            ),
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
    merge_upload_chunks(&plan, backup.backup_root()).map_err(|error| {
        let _ = backup
            .service()
            .jobs()
            .fail(&request.upload_id, error.to_string());
        map_backup_error(error)
    })?;
    let _ = uploads.abort(&request.upload_id);
    backup
        .service()
        .jobs()
        .complete(&request.upload_id, "backup upload completed")
        .map_err(map_backup_error)?;

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
        .map(|session| {
            let _ = fs::remove_dir_all(session.chunk_dir);
            true
        })
        .unwrap_or(false);
    if aborted {
        backup
            .service()
            .jobs()
            .cancel(&request.upload_id, "backup upload cancelled")
            .map_err(map_backup_error)?;
    }

    Ok(Json(ManagementBackupAbortResponse { aborted }))
}

pub async fn file_catalog(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementBackupFileCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let mut files = Vec::new();
    match fs::read_dir(backup.backup_root()) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                if !metadata.is_file() {
                    continue;
                }
                let Some(filename) = entry.file_name().to_str().map(str::to_string) else {
                    continue;
                };
                files.push(ManagementBackupFileInfo {
                    filename,
                    size_bytes: metadata.len(),
                    modified_at_unix: metadata.modified().ok().and_then(system_time_unix_secs),
                });
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(map_backup_error(astrbot_core::AstrbotError::Pipeline(
                error.to_string(),
            )));
        }
    }
    files.sort_by(|left, right| left.filename.cmp(&right.filename));

    Ok(Json(ManagementBackupFileCatalogResponse { files }))
}

pub async fn file_download(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupFileRequest>,
) -> Result<Json<ManagementBackupFileDownloadResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let downloads = state
        .file_downloads()
        .ok_or_else(file_downloads_unavailable)?;
    let filename = validated_backup_filename(&request.filename)?;
    let path = backup_file_path(backup.backup_root(), &filename)?;
    ensure_backup_file_exists(&path)?;

    let now = now_unix();
    let token = backup_file_token(&filename, now);
    let expires_at_unix = now + 900;
    downloads
        .repository()
        .put_file_token(
            FileTokenRecord::new(&token, &path, FileTokenScope::Backup)
                .with_filename(&filename)
                .with_content_type(content_type_for_backup_file(&filename))
                .expires_at_unix(expires_at_unix),
        )
        .await
        .map_err(map_backup_error)?;

    Ok(Json(ManagementBackupFileDownloadResponse {
        filename,
        token: token.clone(),
        download_url: format!("/api/management/files/{token}"),
        expires_at_unix,
    }))
}

pub async fn file_rename(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupFileRenameRequest>,
) -> Result<Json<ManagementBackupFileRenameResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let filename = validated_backup_filename(&request.filename)?;
    let new_filename = validated_backup_filename(&request.new_filename)?;
    let source = backup_file_path(backup.backup_root(), &filename)?;
    ensure_backup_file_exists(&source)?;
    let target = backup_file_path(backup.backup_root(), &new_filename)?;
    if target.exists() {
        return Err(backup_bad_request(format!(
            "backup file {new_filename} already exists"
        )));
    }
    fs::rename(&source, &target).map_err(map_io_backup_error)?;

    Ok(Json(ManagementBackupFileRenameResponse {
        file: backup_file_info(&target, new_filename)?,
    }))
}

pub async fn file_delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupFileRequest>,
) -> Result<Json<ManagementBackupFileDeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let filename = validated_backup_filename(&request.filename)?;
    let path = backup_file_path(backup.backup_root(), &filename)?;
    if !path.exists() {
        return Ok(Json(ManagementBackupFileDeleteResponse { deleted: false }));
    }
    ensure_backup_file_exists(&path)?;
    fs::remove_file(path).map_err(map_io_backup_error)?;

    Ok(Json(ManagementBackupFileDeleteResponse { deleted: true }))
}

pub async fn file_restore(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupFileRestoreRequest>,
) -> Result<Json<ManagementBackupJobResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !request.confirmed {
        return Err(backup_bad_request(
            "backup restore must be confirmed".to_string(),
        ));
    }

    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let filename = validated_backup_filename(&request.filename)?;
    let path = backup_file_path(backup.backup_root(), &filename)?;
    ensure_backup_file_exists(&path)?;
    let task = backup
        .service()
        .start_import(BackupImportJobRequest::new(
            request.task_id,
            filename,
            request.mode,
        ))
        .await
        .map_err(map_backup_error)?;

    Ok(Json(ManagementBackupJobResponse { task }))
}

pub async fn legacy_list(
    State(state): State<ManagementApiState>,
    Query(query): Query<LegacyBackupListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).max(1);
    let mut files = backup_file_catalog(backup.backup_root())?;
    files.sort_by(|left, right| right.modified_at_unix.cmp(&left.modified_at_unix));
    let total = files.len();
    let start = page.saturating_sub(1).saturating_mul(page_size).min(total);
    let end = start.saturating_add(page_size).min(total);
    let items = files[start..end]
        .iter()
        .map(backup_file_to_source)
        .collect::<Vec<_>>();

    Ok(source_ok(json!({
        "items": items,
        "total": total,
        "page": page,
        "page_size": page_size,
    })))
}

pub async fn legacy_export(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let task_id = format!("export-{}", now_unix());
    let task = backup
        .service()
        .start_export(BackupExportJobRequest::new(
            task_id,
            "v4.0.0",
            format!("unix:{}", now_unix()),
        ))
        .await
        .map_err(map_backup_error)?;
    Ok(source_ok(backup_task_to_source(&task)))
}

pub async fn legacy_upload(
    State(state): State<ManagementApiState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let mut original_filename = None;
    let mut bytes = None;
    while let Some(field) = multipart.next_field().await.map_err(map_multipart_error)? {
        if field.name() == Some("file") {
            original_filename = field.file_name().map(str::to_string);
            bytes = Some(field.bytes().await.map_err(map_multipart_error)?.to_vec());
            break;
        }
    }
    let original_filename = original_filename
        .ok_or_else(|| backup_bad_request("backup file is required".to_string()))?;
    if !original_filename.ends_with(".zip") {
        return Err(backup_bad_request(
            "backup upload only accepts .zip files".to_string(),
        ));
    }
    let bytes = bytes.ok_or_else(|| backup_bad_request("backup file is required".to_string()))?;
    let filename = unique_backup_filename(&original_filename, now_unix());
    let path = backup_file_path(backup.backup_root(), &filename)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(map_io_backup_error)?;
    }
    fs::write(&path, &bytes).map_err(map_io_backup_error)?;

    Ok(source_ok(json!({
        "filename": filename,
        "original_filename": original_filename,
        "size": bytes.len(),
    })))
}

pub async fn legacy_upload_init(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyBackupUploadInitRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let upload_id = format!("upload-{}", now_unix());
    let chunk_dir = backup.chunk_root().join(&upload_id);
    fs::create_dir_all(&chunk_dir).map_err(map_io_backup_error)?;
    let total_chunks = total_chunks(request.total_size)?;
    let session = backup
        .uploads
        .lock()
        .map_err(upload_lock_error)?
        .start_upload(
            BackupUploadStart {
                upload_id: upload_id.clone(),
                original_filename: request.filename,
                total_size: request.total_size,
                total_chunks,
                chunk_dir,
            },
            now_unix(),
        )
        .map_err(map_backup_error)?;
    backup
        .service()
        .jobs()
        .create(
            &session.upload_id,
            BackupJobKind::Upload,
            BackupProgressSnapshot::running(
                "upload",
                0,
                u64::from(session.total_chunks),
                "backup upload started",
            ),
        )
        .map_err(map_backup_error)?;

    Ok(source_ok(json!({
        "upload_id": session.upload_id,
        "chunk_size": BACKUP_UPLOAD_CHUNK_SIZE,
        "total_chunks": session.total_chunks,
        "filename": session.filename,
    })))
}

pub async fn legacy_upload_chunk(
    State(state): State<ManagementApiState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let mut upload_id = String::new();
    let mut chunk_index = None;
    let mut chunk_bytes = None;
    while let Some(field) = multipart.next_field().await.map_err(map_multipart_error)? {
        match field.name() {
            Some("upload_id") => {
                upload_id = field.text().await.map_err(map_multipart_error)?;
            }
            Some("chunk_index") => {
                let value = field.text().await.map_err(map_multipart_error)?;
                chunk_index = value.parse::<u32>().ok();
            }
            Some("chunk") => {
                chunk_bytes = Some(field.bytes().await.map_err(map_multipart_error)?.to_vec());
            }
            _ => {}
        }
    }
    let chunk_index =
        chunk_index.ok_or_else(|| backup_bad_request("chunk_index is required".to_string()))?;
    let chunk_bytes =
        chunk_bytes.ok_or_else(|| backup_bad_request("chunk field is required".to_string()))?;
    if chunk_bytes.len() as u64 > BACKUP_UPLOAD_CHUNK_SIZE {
        return Err(backup_bad_request(format!(
            "backup upload chunk exceeds {BACKUP_UPLOAD_CHUNK_SIZE} bytes"
        )));
    }
    let bytes_len = chunk_bytes.len() as u64;
    let receipt = backup
        .uploads
        .lock()
        .map_err(upload_lock_error)?
        .receive_chunk(&upload_id, chunk_index, bytes_len, now_unix())
        .map_err(map_backup_error)?;
    let chunk_path = backup
        .uploads
        .lock()
        .map_err(upload_lock_error)?
        .session(&upload_id)
        .map(|session| session.chunk_dir.join(format!("{chunk_index}.part")))
        .ok_or_else(|| backup_bad_request("backup upload session was not found".to_string()))?;
    if let Some(parent) = chunk_path.parent() {
        fs::create_dir_all(parent).map_err(map_io_backup_error)?;
    }
    fs::write(chunk_path, chunk_bytes).map_err(map_io_backup_error)?;
    backup
        .service()
        .jobs()
        .update_progress(
            &upload_id,
            BackupProgressSnapshot::running(
                "upload",
                u64::from(receipt.received_chunks),
                u64::from(receipt.total_chunks),
                "backup upload chunk received",
            ),
        )
        .map_err(map_backup_error)?;

    Ok(source_ok(json!({
        "received": receipt.received_chunks,
        "total": receipt.total_chunks,
        "chunk_index": receipt.chunk_index,
    })))
}

pub async fn legacy_upload_complete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupCompleteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let mut uploads = backup.uploads.lock().map_err(upload_lock_error)?;
    let plan = uploads
        .complete_plan(&request.upload_id)
        .map_err(map_backup_error)?;
    merge_upload_chunks(&plan, backup.backup_root()).map_err(|error| {
        let _ = backup
            .service()
            .jobs()
            .fail(&request.upload_id, error.to_string());
        map_backup_error(error)
    })?;
    let _ = uploads.abort(&request.upload_id);
    backup
        .service()
        .jobs()
        .complete(&request.upload_id, "backup upload completed")
        .map_err(map_backup_error)?;

    Ok(source_ok(json!({
        "filename": plan.filename,
        "size": plan.total_size,
    })))
}

pub async fn legacy_upload_abort(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupAbortRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let aborted = backup
        .uploads
        .lock()
        .map_err(upload_lock_error)?
        .abort(&request.upload_id)
        .map(|session| {
            let _ = fs::remove_dir_all(session.chunk_dir);
            true
        })
        .unwrap_or(false);
    if aborted {
        backup
            .service()
            .jobs()
            .cancel(&request.upload_id, "backup upload cancelled")
            .map_err(map_backup_error)?;
    }

    Ok(source_ok(json!({ "aborted": aborted })))
}

pub async fn legacy_check(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyBackupCheckRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let filename = validated_backup_filename(&request.filename)?;
    let path = backup_file_path(backup.backup_root(), &filename)?;
    ensure_backup_file_exists(&path)?;
    let precheck = backup
        .service()
        .precheck_import(&BackupImportJobRequest::new(
            "precheck",
            filename,
            BackupImportMode::Merge,
        ))
        .await
        .map_err(map_backup_error)?;

    Ok(source_ok(serde_json::to_value(precheck).map_err(
        |error| map_backup_error(astrbot_core::AstrbotError::Pipeline(error.to_string())),
    )?))
}

pub async fn legacy_import(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyBackupImportRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    if !request.confirmed {
        return Err(backup_bad_request(
            "backup import must be confirmed".to_string(),
        ));
    }
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let filename = validated_backup_filename(&request.filename)?;
    let path = backup_file_path(backup.backup_root(), &filename)?;
    ensure_backup_file_exists(&path)?;
    let task_id = request
        .task_id
        .unwrap_or_else(|| format!("import-{}", now_unix()));
    let task = backup
        .service()
        .start_import(BackupImportJobRequest::new(
            task_id,
            filename,
            BackupImportMode::Merge,
        ))
        .await
        .map_err(map_backup_error)?;

    Ok(source_ok(backup_task_to_source(&task)))
}

pub async fn legacy_progress(
    State(state): State<ManagementApiState>,
    Query(query): Query<LegacyBackupProgressQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let task = backup
        .service()
        .progress_snapshot(&query.task_id)
        .await
        .map_err(map_backup_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("backup task {} was not found", query.task_id),
                }),
            )
        })?;

    Ok(source_ok(backup_task_to_source(&task)))
}

pub async fn legacy_download(
    State(state): State<ManagementApiState>,
    Query(query): Query<LegacyBackupDownloadQuery>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let _ = query.token.as_deref();
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let filename = validated_backup_filename(&query.filename)?;
    let path = backup_file_path(backup.backup_root(), &filename)?;
    ensure_backup_file_exists(&path)?;
    let bytes = fs::read(path).map_err(map_io_backup_error)?;
    let mut response = Body::from(bytes).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        content_type_for_backup_file(&filename)
            .parse()
            .expect("static content type should parse"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{filename}\"")
            .parse()
            .map_err(|error| {
                map_backup_error(astrbot_core::AstrbotError::Pipeline(format!(
                    "backup download header: {error}"
                )))
            })?,
    );
    Ok(response)
}

pub async fn legacy_delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementBackupFileRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let filename = validated_backup_filename(&request.filename)?;
    let path = backup_file_path(backup.backup_root(), &filename)?;
    ensure_backup_file_exists(&path)?;
    fs::remove_file(path).map_err(map_io_backup_error)?;
    Ok(source_ok(json!({})))
}

pub async fn legacy_rename(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyBackupRenameRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let backup = state.backup().ok_or_else(backup_state_unavailable)?;
    let filename = validated_backup_filename(&request.filename)?;
    let mut new_filename = secure_backup_filename(&request.new_name);
    if !new_filename.ends_with(".zip") {
        new_filename.push_str(".zip");
    }
    let source = backup_file_path(backup.backup_root(), &filename)?;
    ensure_backup_file_exists(&source)?;
    let target = backup_file_path(backup.backup_root(), &new_filename)?;
    if target.exists() {
        return Err(backup_bad_request(format!(
            "backup file {new_filename} already exists"
        )));
    }
    fs::rename(source, &target).map_err(map_io_backup_error)?;
    Ok(source_ok(json!({
        "old_filename": filename,
        "new_filename": new_filename,
    })))
}

fn backup_file_catalog(
    root: &std::path::Path,
) -> Result<Vec<ManagementBackupFileInfo>, (StatusCode, Json<ErrorResponse>)> {
    let mut files = Vec::new();
    match fs::read_dir(root) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                if !metadata.is_file() {
                    continue;
                }
                let Some(filename) = entry.file_name().to_str().map(str::to_string) else {
                    continue;
                };
                files.push(ManagementBackupFileInfo {
                    filename,
                    size_bytes: metadata.len(),
                    modified_at_unix: metadata.modified().ok().and_then(system_time_unix_secs),
                });
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(map_io_backup_error(error)),
    }
    Ok(files)
}

fn backup_file_to_source(file: &ManagementBackupFileInfo) -> Value {
    json!({
        "filename": file.filename,
        "size": file.size_bytes,
        "size_bytes": file.size_bytes,
        "created_at": file.modified_at_unix,
        "modified_at_unix": file.modified_at_unix,
        "type": "exported",
        "astrbot_version": null,
        "exported_at": null,
    })
}

fn backup_task_to_source(task: &BackupJobSnapshot) -> Value {
    let status = backup_status_to_source(&task.progress.status);
    json!({
        "task_id": task.task_id,
        "type": backup_kind_to_source(&task.kind),
        "status": status,
        "progress": {
            "status": status,
            "stage": task.progress.stage,
            "current": task.progress.current,
            "total": task.progress.total,
            "message": task.progress.message,
        },
        "result": if matches!(task.progress.status, BackupJobStatus::Completed) {
            json!({ "message": task.progress.message })
        } else {
            Value::Null
        },
        "error": task.error,
    })
}

fn backup_kind_to_source(kind: &BackupJobKind) -> &'static str {
    match kind {
        BackupJobKind::Export => "export",
        BackupJobKind::Import => "import",
        BackupJobKind::Upload => "upload",
        BackupJobKind::Precheck => "precheck",
    }
}

fn backup_status_to_source(status: &BackupJobStatus) -> &'static str {
    match status {
        BackupJobStatus::Queued => "pending",
        BackupJobStatus::Running => "processing",
        BackupJobStatus::Completed => "completed",
        BackupJobStatus::Failed => "failed",
        BackupJobStatus::Cancelled => "cancelled",
    }
}

fn unique_backup_filename(original_filename: &str, timestamp_unix: u64) -> String {
    let secured = secure_backup_filename(original_filename);
    let (stem, extension) = secured
        .rsplit_once('.')
        .map(|(stem, extension)| (stem.to_string(), format!(".{extension}")))
        .unwrap_or_else(|| (secured, ".zip".to_string()));
    secure_backup_filename(&format!("{stem}_{timestamp_unix}{extension}"))
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": null,
        "data": data,
    }))
}

fn backup_state_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "management backup state is not configured".to_string(),
        }),
    )
}

fn file_downloads_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "management file downloads are not configured".to_string(),
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

fn backup_bad_request(message: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse { error: message }),
    )
}

fn map_io_backup_error(error: std::io::Error) -> (StatusCode, Json<ErrorResponse>) {
    map_backup_error(astrbot_core::AstrbotError::Pipeline(error.to_string()))
}

fn map_multipart_error(
    error: axum::extract::multipart::MultipartError,
) -> (StatusCode, Json<ErrorResponse>) {
    backup_bad_request(format!("backup multipart upload: {error}"))
}

fn validated_backup_filename(filename: &str) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let trimmed = filename.trim();
    let secured = secure_backup_filename(trimmed);
    if trimmed.is_empty() || secured != trimmed {
        return Err(backup_bad_request(
            "backup filename must be a direct safe filename".to_string(),
        ));
    }
    Ok(secured)
}

fn secure_backup_filename(filename: &str) -> String {
    let normalized = filename.replace('\\', "/");
    let basename = normalized.rsplit('/').next().unwrap_or_default();
    let without_parent = basename.replace("..", "");
    let cleaned = without_parent
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        .collect::<String>();
    let trimmed = cleaned.trim_matches('.');
    if trimmed.is_empty() {
        "backup".to_string()
    } else {
        trimmed.to_string()
    }
}

fn backup_file_path(
    root: &std::path::Path,
    filename: &str,
) -> Result<PathBuf, (StatusCode, Json<ErrorResponse>)> {
    let path = root.join(filename);
    if path.parent() != Some(root) {
        return Err(backup_bad_request(
            "backup filename must stay within backup root".to_string(),
        ));
    }
    Ok(path)
}

fn ensure_backup_file_exists(
    path: &std::path::Path,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if path.is_file() {
        Ok(())
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "backup file was not found".to_string(),
            }),
        ))
    }
}

fn backup_file_info(
    path: &std::path::Path,
    filename: String,
) -> Result<ManagementBackupFileInfo, (StatusCode, Json<ErrorResponse>)> {
    let metadata = fs::metadata(path).map_err(map_io_backup_error)?;
    Ok(ManagementBackupFileInfo {
        filename,
        size_bytes: metadata.len(),
        modified_at_unix: metadata.modified().ok().and_then(system_time_unix_secs),
    })
}

fn content_type_for_backup_file(filename: &str) -> &'static str {
    if filename.ends_with(".zip") {
        "application/zip"
    } else if filename.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

fn backup_file_token(filename: &str, now_unix: u64) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(u128::from(now_unix));
    let label = filename
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
    format!("backup-{timestamp}-{label}")
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn system_time_unix_secs(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
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

fn decode_chunk_bytes(
    request: &ManagementBackupChunkRequest,
) -> Result<Vec<u8>, (StatusCode, Json<ErrorResponse>)> {
    if request.bytes_len > BACKUP_UPLOAD_CHUNK_SIZE {
        return Err(backup_bad_request(format!(
            "backup upload chunk exceeds {BACKUP_UPLOAD_CHUNK_SIZE} bytes"
        )));
    }
    if let Some(bytes_base64) = &request.bytes_base64 {
        BASE64_STANDARD
            .decode(bytes_base64)
            .map_err(|error| backup_bad_request(format!("invalid backup chunk base64: {error}")))
    } else {
        usize::try_from(request.bytes_len)
            .map(|len| vec![0_u8; len])
            .map_err(|_| backup_bad_request("backup chunk is too large".to_string()))
    }
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
