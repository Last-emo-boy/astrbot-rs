use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_storage::{FileTokenRecord, FileTokenRepository, FileTokenScope};
use axum::{
    Json,
    body::Body,
    extract::{Path as AxumPath, State},
    http::{
        HeaderValue, StatusCode,
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
    },
    response::Response,
};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone)]
pub struct ManagementFileDownloadState {
    repository: Arc<dyn FileTokenRepository>,
    allowed_scopes: BTreeSet<FileTokenScope>,
}

impl ManagementFileDownloadState {
    pub fn new(repository: Arc<dyn FileTokenRepository>) -> Self {
        Self {
            repository,
            allowed_scopes: [
                FileTokenScope::Dashboard,
                FileTokenScope::Plugin,
                FileTokenScope::Backup,
            ]
            .into_iter()
            .collect(),
        }
    }

    pub fn with_allowed_scopes(mut self, scopes: impl IntoIterator<Item = FileTokenScope>) -> Self {
        self.allowed_scopes = scopes.into_iter().collect();
        self
    }

    pub fn repository(&self) -> Arc<dyn FileTokenRepository> {
        self.repository.clone()
    }

    pub fn is_scope_allowed(&self, scope: &FileTokenScope) -> bool {
        self.allowed_scopes.contains(scope)
    }
}

impl fmt::Debug for ManagementFileDownloadState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagementFileDownloadState")
            .field("allowed_scopes", &self.allowed_scopes)
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

        let filename = record.filename.or_else(|| {
            record
                .file_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        });

        Ok(ScopedDownloadFile {
            path: record.file_path,
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
    MissingFile,
    Storage(String),
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
    let bytes =
        fs::read(&file.path).map_err(|_| map_download_error(ScopedDownloadError::MissingFile))?;

    let mut response = Response::new(Body::from(bytes));
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
        ScopedDownloadError::ScopeDenied => StatusCode::FORBIDDEN,
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
                ScopedDownloadError::MissingFile => "file is no longer available".to_string(),
                ScopedDownloadError::Storage(message) => message,
            },
        }),
    )
}

fn content_disposition(filename: &str) -> String {
    let filename = filename.replace(['"', '\r', '\n'], "_");
    format!("attachment; filename=\"{filename}\"")
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
