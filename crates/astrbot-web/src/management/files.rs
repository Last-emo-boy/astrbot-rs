use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_storage::{
    AttachmentRecord, AttachmentRepository, FileTokenRecord, FileTokenRepository, FileTokenScope,
};
use axum::{
    Json,
    body::{Body, Bytes},
    extract::{
        Multipart, Path as AxumPath, State,
        multipart::{Field, MultipartError},
    },
    http::{
        HeaderValue, StatusCode,
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
    },
    response::Response,
};
use rand::{Rng, distributions::Alphanumeric};
use serde::Serialize;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::ErrorResponse;

use super::ManagementApiState;

const DEFAULT_TOKEN_TTL_SECONDS: u64 = 60 * 60;
const FILE_STREAM_CHUNK_SIZE: usize = 64 * 1024;

#[derive(Clone)]
pub struct ManagementFileDownloadState {
    repository: Arc<dyn FileTokenRepository>,
    attachment_repository: Option<Arc<dyn AttachmentRepository>>,
    allowed_scopes: BTreeSet<FileTokenScope>,
    allowed_roots: Vec<PathBuf>,
    attachment_dir: Option<PathBuf>,
    temp_dir: Option<PathBuf>,
    default_token_ttl_seconds: u64,
}

impl ManagementFileDownloadState {
    pub fn new(repository: Arc<dyn FileTokenRepository>) -> Self {
        Self {
            repository,
            attachment_repository: None,
            allowed_scopes: [
                FileTokenScope::Dashboard,
                FileTokenScope::Plugin,
                FileTokenScope::Backup,
                FileTokenScope::Attachment,
                FileTokenScope::OpenApiFile,
            ]
            .into_iter()
            .collect(),
            allowed_roots: Vec::new(),
            attachment_dir: None,
            temp_dir: None,
            default_token_ttl_seconds: DEFAULT_TOKEN_TTL_SECONDS,
        }
    }

    pub fn with_attachment_repository(mut self, repository: Arc<dyn AttachmentRepository>) -> Self {
        self.attachment_repository = Some(repository);
        self
    }

    pub fn with_file_roots(
        mut self,
        attachment_dir: impl Into<PathBuf>,
        temp_dir: impl Into<PathBuf>,
    ) -> Self {
        let attachment_dir = attachment_dir.into();
        let temp_dir = temp_dir.into();
        push_unique_path(&mut self.allowed_roots, attachment_dir.clone());
        push_unique_path(&mut self.allowed_roots, temp_dir.clone());
        self.attachment_dir = Some(attachment_dir);
        self.temp_dir = Some(temp_dir);
        self
    }

    pub fn with_allowed_root(mut self, root: impl Into<PathBuf>) -> Self {
        push_unique_path(&mut self.allowed_roots, root.into());
        self
    }

    pub fn with_allowed_roots(mut self, roots: impl IntoIterator<Item = PathBuf>) -> Self {
        for root in roots {
            push_unique_path(&mut self.allowed_roots, root);
        }
        self
    }

    pub fn with_default_token_ttl_seconds(mut self, ttl_seconds: u64) -> Self {
        self.default_token_ttl_seconds = ttl_seconds;
        self
    }

    pub fn with_allowed_scopes(mut self, scopes: impl IntoIterator<Item = FileTokenScope>) -> Self {
        self.allowed_scopes = scopes.into_iter().collect();
        self
    }

    pub fn repository(&self) -> Arc<dyn FileTokenRepository> {
        self.repository.clone()
    }

    pub fn attachment_repository(&self) -> Option<Arc<dyn AttachmentRepository>> {
        self.attachment_repository.clone()
    }

    pub fn is_scope_allowed(&self, scope: &FileTokenScope) -> bool {
        self.allowed_scopes.contains(scope)
    }

    fn upload_paths(&self) -> Result<(PathBuf, PathBuf), FileUploadError> {
        let attachment_dir = self
            .attachment_dir
            .clone()
            .ok_or(FileUploadError::UploadStorageUnavailable)?;
        let temp_dir = self
            .temp_dir
            .clone()
            .ok_or(FileUploadError::UploadStorageUnavailable)?;
        Ok((attachment_dir, temp_dir))
    }

    fn authorize_existing_file(&self, path: &Path) -> Result<PathBuf, ScopedDownloadError> {
        let canonical = fs::canonicalize(path).map_err(|_| ScopedDownloadError::MissingFile)?;
        if self.allowed_roots.is_empty() {
            return Ok(canonical);
        }
        for root in &self.allowed_roots {
            if let Ok(root) = fs::canonicalize(root)
                && canonical.starts_with(root)
            {
                return Ok(canonical);
            }
        }
        Err(ScopedDownloadError::PathDenied)
    }
}

impl fmt::Debug for ManagementFileDownloadState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagementFileDownloadState")
            .field("allowed_scopes", &self.allowed_scopes)
            .field("allowed_roots", &self.allowed_roots)
            .field("attachment_dir", &self.attachment_dir)
            .field("temp_dir", &self.temp_dir)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopedDownloadFile {
    pub path: PathBuf,
    pub scope: FileTokenScope,
    pub filename: Option<String>,
    pub content_type: Option<String>,
}

pub struct ScopedDownloadService<'a> {
    state: &'a ManagementFileDownloadState,
}

impl<'a> ScopedDownloadService<'a> {
    pub fn new(state: &'a ManagementFileDownloadState) -> Self {
        Self { state }
    }

    pub async fn consume(&self, token: &str) -> Result<ScopedDownloadFile, ScopedDownloadError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(ScopedDownloadError::InvalidToken);
        }

        let Some(record) = self
            .state
            .repository
            .consume_file_token(token, now_unix())
            .await
            .map_err(|error| ScopedDownloadError::Storage(error.to_string()))?
        else {
            return Err(ScopedDownloadError::InvalidToken);
        };

        self.file_from_record(record)
    }

    fn file_from_record(
        &self,
        record: FileTokenRecord,
    ) -> Result<ScopedDownloadFile, ScopedDownloadError> {
        if !self.state.is_scope_allowed(&record.scope) {
            return Err(ScopedDownloadError::ScopeDenied);
        }
        if !record.file_path.is_file() {
            return Err(ScopedDownloadError::MissingFile);
        }
        let path = self.state.authorize_existing_file(&record.file_path)?;

        let filename = record.filename.or_else(|| {
            record
                .file_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        });

        Ok(ScopedDownloadFile {
            path,
            scope: record.scope,
            filename,
            content_type: record.content_type,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScopedDownloadError {
    InvalidToken,
    ScopeDenied,
    PathDenied,
    MissingFile,
    Storage(String),
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ManagementFileUploadResponse {
    pub attachment_id: String,
    pub token: String,
    pub download_url: String,
    pub filename: String,
    pub content_type: Option<String>,
    pub scope: String,
    pub size_bytes: u64,
    pub expires_at_unix: Option<u64>,
    pub single_use: bool,
    pub stored_url: String,
}

pub struct FileUploadService<'a> {
    state: &'a ManagementFileDownloadState,
}

impl<'a> FileUploadService<'a> {
    pub fn new(state: &'a ManagementFileDownloadState) -> Self {
        Self { state }
    }

    pub async fn upload_multipart(
        &self,
        multipart: &mut Multipart,
    ) -> Result<ManagementFileUploadResponse, FileUploadError> {
        let mut form = UploadForm::default();
        let mut saved_upload = None;

        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(FileUploadError::multipart)?
        {
            let name = field.name().map(str::to_string);
            let filename = field.file_name().map(str::to_string);
            let content_type = field.content_type().map(ToString::to_string);
            if name.as_deref() == Some("file") || filename.is_some() {
                if saved_upload.is_some() {
                    return Err(FileUploadError::InvalidInput(
                        "only one upload file is supported".to_string(),
                    ));
                }
                saved_upload = Some(
                    self.save_upload_field(field, filename, content_type)
                        .await?,
                );
            } else if let Some(name) = name {
                let value = field.text().await.map_err(FileUploadError::multipart)?;
                form.apply_text_field(&name, value)?;
            }
        }

        let Some(saved_upload) = saved_upload else {
            return Err(FileUploadError::MissingFilePart);
        };
        let temp_path = saved_upload.temp_path.clone();
        match self.finish_upload(saved_upload, form).await {
            Ok(response) => Ok(response),
            Err(error) => {
                let _ = fs::remove_file(temp_path);
                Err(error)
            }
        }
    }

    async fn save_upload_field(
        &self,
        mut field: Field<'_>,
        filename: Option<String>,
        content_type: Option<String>,
    ) -> Result<SavedUpload, FileUploadError> {
        let (_, temp_dir) = self.state.upload_paths()?;
        fs::create_dir_all(&temp_dir).map_err(FileUploadError::io)?;
        let temp_path = temp_dir.join(format!(
            "upload-{}-{}.tmp",
            unix_nanos(),
            random_alphanumeric(12)
        ));
        let mut file = File::create(&temp_path).map_err(FileUploadError::io)?;
        let mut size_bytes = 0_u64;
        while let Some(chunk) = field.chunk().await.map_err(FileUploadError::multipart)? {
            file.write_all(&chunk).map_err(FileUploadError::io)?;
            size_bytes = size_bytes.saturating_add(chunk.len() as u64);
        }
        file.flush().map_err(FileUploadError::io)?;
        Ok(SavedUpload {
            temp_path,
            filename,
            content_type,
            size_bytes,
        })
    }

    async fn finish_upload(
        &self,
        upload: SavedUpload,
        form: UploadForm,
    ) -> Result<ManagementFileUploadResponse, FileUploadError> {
        let attachments = self
            .state
            .attachment_repository()
            .ok_or(FileUploadError::UploadStorageUnavailable)?;
        let (attachment_dir, _) = self.state.upload_paths()?;
        fs::create_dir_all(&attachment_dir).map_err(FileUploadError::io)?;

        let now = now_unix();
        let attachment_id = form
            .attachment_id
            .and_then(safe_identifier)
            .unwrap_or_else(|| format!("att-{}-{}", unix_nanos(), random_alphanumeric(8)));
        let filename = form
            .filename
            .or(upload.filename)
            .and_then(safe_filename)
            .unwrap_or_else(|| "upload.bin".to_string());
        let content_type = form.content_type.or(upload.content_type);
        let scope = form.scope.unwrap_or(FileTokenScope::Attachment);
        let single_use = form.single_use.unwrap_or(true);
        let expires_at_unix = form
            .expires_at_unix
            .or_else(|| form.ttl_seconds.map(|ttl| now.saturating_add(ttl)))
            .or_else(|| Some(now.saturating_add(self.state.default_token_ttl_seconds)));

        let final_path = attachment_dir.join(format!("{attachment_id}-{filename}"));
        if final_path.exists() {
            return Err(FileUploadError::InvalidInput(
                "attachment upload target already exists".to_string(),
            ));
        }
        fs::rename(&upload.temp_path, &final_path).map_err(FileUploadError::io)?;
        let canonical_path = self
            .state
            .authorize_existing_file(&final_path)
            .map_err(|_| FileUploadError::PathDenied)?;

        let token = format!("file-{}-{}", unix_nanos(), random_alphanumeric(24));
        self.state
            .repository
            .remove_expired_file_tokens(now)
            .await
            .map_err(|error| FileUploadError::Storage(error.to_string()))?;

        let mut token_record =
            FileTokenRecord::new(&token, canonical_path, scope.clone()).with_filename(&filename);
        token_record.content_type = content_type.clone();
        token_record.expires_at_unix = expires_at_unix;
        token_record.single_use = single_use;
        self.state
            .repository
            .put_file_token(token_record)
            .await
            .map_err(|error| FileUploadError::Storage(error.to_string()))?;

        let download_url = format!("/api/management/files/{token}");
        let mut attachment =
            AttachmentRecord::new(&attachment_id, format!("upload://{attachment_id}"))
                .with_stored_url(&download_url);
        attachment.filename = Some(filename.clone());
        attachment.content_type = content_type.clone();
        attachments
            .put_attachment(attachment)
            .await
            .map_err(|error| FileUploadError::Storage(error.to_string()))?;

        Ok(ManagementFileUploadResponse {
            attachment_id,
            token,
            download_url: download_url.clone(),
            filename,
            content_type,
            scope: scope.as_str().to_string(),
            size_bytes: upload.size_bytes,
            expires_at_unix,
            single_use,
            stored_url: download_url,
        })
    }
}

#[derive(Default)]
struct UploadForm {
    attachment_id: Option<String>,
    filename: Option<String>,
    content_type: Option<String>,
    scope: Option<FileTokenScope>,
    expires_at_unix: Option<u64>,
    ttl_seconds: Option<u64>,
    single_use: Option<bool>,
}

impl UploadForm {
    fn apply_text_field(&mut self, name: &str, value: String) -> Result<(), FileUploadError> {
        match name {
            "attachment_id" | "id" => self.attachment_id = non_empty_string(value),
            "filename" | "name" => self.filename = non_empty_string(value),
            "content_type" => self.content_type = non_empty_string(value),
            "scope" => self.scope = Some(upload_scope(&value)?),
            "expires_at_unix" => self.expires_at_unix = Some(parse_u64(name, &value)?),
            "ttl_seconds" => self.ttl_seconds = Some(parse_u64(name, &value)?),
            "single_use" => self.single_use = Some(parse_bool(&value)?),
            _ => {}
        }
        Ok(())
    }
}

struct SavedUpload {
    temp_path: PathBuf,
    filename: Option<String>,
    content_type: Option<String>,
    size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileUploadError {
    UploadStorageUnavailable,
    MissingFilePart,
    InvalidInput(String),
    PathDenied,
    Io(String),
    Multipart(String),
    Storage(String),
}

impl FileUploadError {
    fn io(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }

    fn multipart(error: MultipartError) -> Self {
        Self::Multipart(error.to_string())
    }
}

pub async fn download(
    State(state): State<ManagementApiState>,
    AxumPath(token): AxumPath<String>,
) -> Result<Response<Body>, (StatusCode, Json<ErrorResponse>)> {
    let downloads = state
        .file_downloads()
        .ok_or_else(file_downloads_unavailable)?;
    let file = ScopedDownloadService::new(downloads)
        .consume(&token)
        .await
        .map_err(map_download_error)?;
    let body =
        stream_file(file.path).map_err(|_| map_download_error(ScopedDownloadError::MissingFile))?;

    let mut response = Response::new(body);
    let headers = response.headers_mut();
    if let Ok(content_type) = HeaderValue::from_str(
        file.content_type
            .as_deref()
            .unwrap_or("application/octet-stream"),
    ) {
        headers.insert(CONTENT_TYPE, content_type);
    }
    if let Some(filename) = file.filename.as_deref()
        && let Ok(disposition) = HeaderValue::from_str(&content_disposition(filename))
    {
        headers.insert(CONTENT_DISPOSITION, disposition);
    }

    Ok(response)
}

pub async fn upload(
    State(state): State<ManagementApiState>,
    mut multipart: Multipart,
) -> Result<Json<ManagementFileUploadResponse>, (StatusCode, Json<ErrorResponse>)> {
    let files = state
        .file_downloads()
        .ok_or_else(file_downloads_unavailable)?;
    let response = FileUploadService::new(files)
        .upload_multipart(&mut multipart)
        .await
        .map_err(map_upload_error)?;
    Ok(Json(response))
}

fn file_downloads_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "management file downloads are not configured".to_string(),
        }),
    )
}

fn map_download_error(error: ScopedDownloadError) -> (StatusCode, Json<ErrorResponse>) {
    let status = match error {
        ScopedDownloadError::InvalidToken | ScopedDownloadError::MissingFile => {
            StatusCode::NOT_FOUND
        }
        ScopedDownloadError::ScopeDenied | ScopedDownloadError::PathDenied => StatusCode::FORBIDDEN,
        ScopedDownloadError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(ErrorResponse {
            error: match error {
                ScopedDownloadError::InvalidToken => "file token is invalid or expired".to_string(),
                ScopedDownloadError::ScopeDenied => {
                    "file token scope is not allowed for this route".to_string()
                }
                ScopedDownloadError::PathDenied => {
                    "file path is outside configured download roots".to_string()
                }
                ScopedDownloadError::MissingFile => "file is no longer available".to_string(),
                ScopedDownloadError::Storage(message) => message,
            },
        }),
    )
}

fn map_upload_error(error: FileUploadError) -> (StatusCode, Json<ErrorResponse>) {
    let status = match error {
        FileUploadError::UploadStorageUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        FileUploadError::MissingFilePart | FileUploadError::InvalidInput(_) => {
            StatusCode::BAD_REQUEST
        }
        FileUploadError::PathDenied => StatusCode::FORBIDDEN,
        FileUploadError::Io(_) | FileUploadError::Multipart(_) | FileUploadError::Storage(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    (
        status,
        Json(ErrorResponse {
            error: match error {
                FileUploadError::UploadStorageUnavailable => {
                    "management file upload storage is not configured".to_string()
                }
                FileUploadError::MissingFilePart => {
                    "multipart upload must include a file field".to_string()
                }
                FileUploadError::InvalidInput(message)
                | FileUploadError::Io(message)
                | FileUploadError::Multipart(message)
                | FileUploadError::Storage(message) => message,
                FileUploadError::PathDenied => {
                    "uploaded file path is outside configured roots".to_string()
                }
            },
        }),
    )
}

fn stream_file(path: PathBuf) -> io::Result<Body> {
    let mut file = File::open(path)?;
    let (tx, rx) = mpsc::channel::<Result<Bytes, io::Error>>(4);
    tokio::task::spawn_blocking(move || {
        let mut buffer = vec![0_u8; FILE_STREAM_CHUNK_SIZE];
        loop {
            match file.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    if tx
                        .blocking_send(Ok(Bytes::copy_from_slice(&buffer[..read])))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) => {
                    let _ = tx.blocking_send(Err(error));
                    break;
                }
            }
        }
    });
    Ok(Body::from_stream(ReceiverStream::new(rx)))
}

fn content_disposition(filename: &str) -> String {
    let filename = filename.replace(['"', '\r', '\n'], "_");
    format!("attachment; filename=\"{filename}\"")
}

fn safe_filename(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let normalized = value.replace('\\', "/");
    let name = normalized.rsplit('/').next().unwrap_or_default().trim();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    let sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    non_empty_string(sanitized)
}

fn safe_identifier(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let sanitized = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    non_empty_string(sanitized).filter(|value| value != "." && value != "..")
}

fn upload_scope(scope: &str) -> Result<FileTokenScope, FileUploadError> {
    match FileTokenScope::from(scope) {
        FileTokenScope::Attachment => Ok(FileTokenScope::Attachment),
        FileTokenScope::OpenApiFile => Ok(FileTokenScope::OpenApiFile),
        _ => Err(FileUploadError::InvalidInput(
            "file upload scope must be attachment or openapi.file".to_string(),
        )),
    }
}

fn parse_u64(field: &str, value: &str) -> Result<u64, FileUploadError> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| FileUploadError::InvalidInput(format!("{field} must be an unsigned integer")))
}

fn parse_bool(value: &str) -> Result<bool, FileUploadError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(FileUploadError::InvalidInput(
            "single_use must be a boolean".to_string(),
        )),
    }
}

fn non_empty_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn random_alphanumeric(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}
