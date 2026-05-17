use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};

pub const BACKUP_UPLOAD_CHUNK_SIZE: u64 = 1024 * 1024;
pub const BACKUP_UPLOAD_EXPIRE_SECONDS: u64 = 3600;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupUploadStart {
    pub upload_id: String,
    pub original_filename: String,
    pub total_size: u64,
    pub total_chunks: u32,
    pub chunk_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupUploadSession {
    pub upload_id: String,
    pub filename: String,
    pub original_filename: String,
    pub total_size: u64,
    pub total_chunks: u32,
    pub received_chunks: BTreeSet<u32>,
    pub created_at_unix: u64,
    pub last_activity_unix: u64,
    pub chunk_dir: PathBuf,
}

impl BackupUploadSession {
    pub fn received_count(&self) -> u32 {
        self.received_chunks.len() as u32
    }

    pub fn is_complete(&self) -> bool {
        self.received_count() == self.total_chunks
    }

    pub fn is_expired_at(&self, now_unix: u64) -> bool {
        now_unix.saturating_sub(self.last_activity_unix) >= BACKUP_UPLOAD_EXPIRE_SECONDS
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupChunkReceipt {
    pub upload_id: String,
    pub chunk_index: u32,
    pub received_chunks: u32,
    pub total_chunks: u32,
    pub complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupUploadCompletePlan {
    pub upload_id: String,
    pub filename: String,
    pub chunk_dir: PathBuf,
    pub ordered_chunk_indexes: Vec<u32>,
    pub total_size: u64,
}

#[derive(Debug, Default)]
pub struct BackupUploadManager {
    sessions: BTreeMap<String, BackupUploadSession>,
}

impl BackupUploadManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_upload(
        &mut self,
        request: BackupUploadStart,
        now_unix: u64,
    ) -> Result<BackupUploadSession> {
        let upload_id = non_empty(request.upload_id, "backup upload id")?;
        if request.total_chunks == 0 {
            return Err(AstrbotError::Pipeline(
                "backup upload total_chunks must be greater than zero".to_string(),
            ));
        }
        let filename = secure_backup_filename(&request.original_filename);
        let session = BackupUploadSession {
            upload_id: upload_id.clone(),
            filename,
            original_filename: request.original_filename,
            total_size: request.total_size,
            total_chunks: request.total_chunks,
            received_chunks: BTreeSet::new(),
            created_at_unix: now_unix,
            last_activity_unix: now_unix,
            chunk_dir: request.chunk_dir,
        };
        self.sessions.insert(upload_id, session.clone());
        Ok(session)
    }

    pub fn receive_chunk(
        &mut self,
        upload_id: &str,
        chunk_index: u32,
        bytes_len: u64,
        now_unix: u64,
    ) -> Result<BackupChunkReceipt> {
        if bytes_len > BACKUP_UPLOAD_CHUNK_SIZE {
            return Err(AstrbotError::Pipeline(format!(
                "backup upload chunk exceeds {} bytes",
                BACKUP_UPLOAD_CHUNK_SIZE
            )));
        }

        let upload_id = upload_id.trim();
        let Some(session) = self.sessions.get_mut(upload_id) else {
            return Err(AstrbotError::Pipeline(format!(
                "backup upload session {upload_id} was not found"
            )));
        };
        if session.is_expired_at(now_unix) {
            return Err(AstrbotError::Pipeline(format!(
                "backup upload session {upload_id} expired"
            )));
        }
        if chunk_index >= session.total_chunks {
            return Err(AstrbotError::Pipeline(format!(
                "backup upload chunk {chunk_index} is out of range"
            )));
        }

        session.received_chunks.insert(chunk_index);
        session.last_activity_unix = now_unix;
        Ok(BackupChunkReceipt {
            upload_id: session.upload_id.clone(),
            chunk_index,
            received_chunks: session.received_count(),
            total_chunks: session.total_chunks,
            complete: session.is_complete(),
        })
    }

    pub fn complete_plan(&self, upload_id: &str) -> Result<BackupUploadCompletePlan> {
        let upload_id = upload_id.trim();
        let Some(session) = self.sessions.get(upload_id) else {
            return Err(AstrbotError::Pipeline(format!(
                "backup upload session {upload_id} was not found"
            )));
        };
        if !session.is_complete() {
            let missing = (0..session.total_chunks)
                .filter(|index| !session.received_chunks.contains(index))
                .map(|index| index.to_string())
                .collect::<Vec<_>>()
                .join(",");
            return Err(AstrbotError::Pipeline(format!(
                "backup upload session {upload_id} is missing chunks: {missing}"
            )));
        }

        Ok(BackupUploadCompletePlan {
            upload_id: session.upload_id.clone(),
            filename: session.filename.clone(),
            chunk_dir: session.chunk_dir.clone(),
            ordered_chunk_indexes: (0..session.total_chunks).collect(),
            total_size: session.total_size,
        })
    }

    pub fn abort(&mut self, upload_id: &str) -> Option<BackupUploadSession> {
        self.sessions.remove(upload_id.trim())
    }

    pub fn cleanup_expired(&mut self, now_unix: u64) -> Vec<BackupUploadSession> {
        let expired = self
            .sessions
            .iter()
            .filter(|(_, session)| session.is_expired_at(now_unix))
            .map(|(upload_id, _)| upload_id.clone())
            .collect::<Vec<_>>();

        expired
            .into_iter()
            .filter_map(|upload_id| self.sessions.remove(&upload_id))
            .collect()
    }

    pub fn session(&self, upload_id: &str) -> Option<&BackupUploadSession> {
        self.sessions.get(upload_id.trim())
    }
}

pub fn secure_backup_filename(filename: &str) -> String {
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

pub fn unique_backup_filename(original_filename: &str, timestamp_unix: u64) -> String {
    let filename = secure_backup_filename(original_filename);
    if let Some(dot) = filename.rfind('.') {
        let (stem, extension) = filename.split_at(dot);
        format!("{stem}_{timestamp_unix}{extension}")
    } else {
        format!("{filename}_{timestamp_unix}")
    }
}

fn non_empty(value: impl Into<String>, field: &str) -> Result<String> {
    let value = value.into();
    let value = value.trim();
    if value.is_empty() {
        return Err(AstrbotError::Pipeline(format!("{field} must not be empty")));
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        BACKUP_UPLOAD_CHUNK_SIZE, BACKUP_UPLOAD_EXPIRE_SECONDS, BackupUploadManager,
        BackupUploadStart, secure_backup_filename, unique_backup_filename,
    };

    #[test]
    fn secure_filename_matches_dashboard_backup_rules() {
        assert_eq!(
            secure_backup_filename("../evil/path\\backup..zip"),
            "backupzip"
        );
        assert_eq!(secure_backup_filename("..."), "backup");
        assert_eq!(
            unique_backup_filename("export.zip", 1778970000),
            "export_1778970000.zip"
        );
    }

    #[test]
    fn chunked_upload_state_and_completion_are_tested_outside_http() {
        let mut uploads = BackupUploadManager::new();
        uploads
            .start_upload(
                BackupUploadStart {
                    upload_id: "upload-1".to_string(),
                    original_filename: "backup.zip".to_string(),
                    total_size: BACKUP_UPLOAD_CHUNK_SIZE + 10,
                    total_chunks: 2,
                    chunk_dir: PathBuf::from("chunks/upload-1"),
                },
                100,
            )
            .expect("upload should start");

        let first = uploads
            .receive_chunk("upload-1", 0, BACKUP_UPLOAD_CHUNK_SIZE, 110)
            .expect("first chunk should store");
        assert!(!first.complete);

        assert!(uploads.complete_plan("upload-1").is_err());

        let second = uploads
            .receive_chunk("upload-1", 1, 10, 120)
            .expect("second chunk should store");
        assert!(second.complete);

        let plan = uploads
            .complete_plan("upload-1")
            .expect("complete upload should produce merge plan");
        assert_eq!(plan.ordered_chunk_indexes, vec![0, 1]);
    }

    #[test]
    fn upload_cleanup_removes_expired_sessions() {
        let mut uploads = BackupUploadManager::new();
        uploads
            .start_upload(
                BackupUploadStart {
                    upload_id: "expired".to_string(),
                    original_filename: "backup.zip".to_string(),
                    total_size: 1,
                    total_chunks: 1,
                    chunk_dir: PathBuf::from("chunks/expired"),
                },
                100,
            )
            .expect("upload should start");

        let removed = uploads.cleanup_expired(100 + BACKUP_UPLOAD_EXPIRE_SECONDS);
        assert_eq!(removed.len(), 1);
        assert!(uploads.session("expired").is_none());
    }
}
