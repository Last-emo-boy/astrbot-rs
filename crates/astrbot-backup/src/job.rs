use std::collections::BTreeMap;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupJobKind {
    Export,
    Import,
    Upload,
    Precheck,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupProgressSnapshot {
    pub status: BackupJobStatus,
    pub stage: String,
    pub current: u64,
    pub total: u64,
    pub message: String,
}

impl BackupProgressSnapshot {
    pub fn new(
        status: BackupJobStatus,
        stage: impl Into<String>,
        current: u64,
        total: u64,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status,
            stage: stage.into(),
            current,
            total,
            message: message.into(),
        }
    }

    pub fn queued(message: impl Into<String>) -> Self {
        Self::new(BackupJobStatus::Queued, "queued", 0, 0, message)
    }

    pub fn running(
        stage: impl Into<String>,
        current: u64,
        total: u64,
        message: impl Into<String>,
    ) -> Self {
        Self::new(BackupJobStatus::Running, stage, current, total, message)
    }

    pub fn completed(message: impl Into<String>) -> Self {
        Self::new(BackupJobStatus::Completed, "completed", 1, 1, message)
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self::new(BackupJobStatus::Failed, "failed", 0, 0, message)
    }

    pub fn cancelled(message: impl Into<String>) -> Self {
        Self::new(BackupJobStatus::Cancelled, "cancelled", 0, 0, message)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupJobSnapshot {
    pub task_id: String,
    pub kind: BackupJobKind,
    pub progress: BackupProgressSnapshot,
    pub error: Option<String>,
}

impl BackupJobSnapshot {
    pub fn new(
        task_id: impl Into<String>,
        kind: BackupJobKind,
        progress: BackupProgressSnapshot,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            kind,
            progress,
            error: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct BackupJobStore {
    jobs: RwLock<BTreeMap<String, BackupJobSnapshot>>,
}

impl BackupJobStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(
        &self,
        task_id: impl Into<String>,
        kind: BackupJobKind,
        progress: BackupProgressSnapshot,
    ) -> Result<BackupJobSnapshot> {
        let task_id = normalize_task_id(task_id)?;
        let snapshot = BackupJobSnapshot::new(task_id.clone(), kind, progress);
        self.jobs
            .write()
            .map_err(lock_error)?
            .insert(task_id, snapshot.clone());
        Ok(snapshot)
    }

    pub fn snapshot(&self, task_id: &str) -> Result<Option<BackupJobSnapshot>> {
        Ok(self
            .jobs
            .read()
            .map_err(lock_error)?
            .get(task_id.trim())
            .cloned())
    }

    pub fn snapshots(&self) -> Result<Vec<BackupJobSnapshot>> {
        Ok(self
            .jobs
            .read()
            .map_err(lock_error)?
            .values()
            .cloned()
            .collect())
    }

    pub fn update_progress(
        &self,
        task_id: &str,
        progress: BackupProgressSnapshot,
    ) -> Result<Option<BackupJobSnapshot>> {
        let mut jobs = self.jobs.write().map_err(lock_error)?;
        let Some(snapshot) = jobs.get_mut(task_id.trim()) else {
            return Ok(None);
        };
        snapshot.progress = progress;
        snapshot.error = None;
        Ok(Some(snapshot.clone()))
    }

    pub fn complete(
        &self,
        task_id: &str,
        message: impl Into<String>,
    ) -> Result<Option<BackupJobSnapshot>> {
        self.update_progress(task_id, BackupProgressSnapshot::completed(message))
    }

    pub fn fail(
        &self,
        task_id: &str,
        message: impl Into<String>,
    ) -> Result<Option<BackupJobSnapshot>> {
        let message = message.into();
        let mut jobs = self.jobs.write().map_err(lock_error)?;
        let Some(snapshot) = jobs.get_mut(task_id.trim()) else {
            return Ok(None);
        };
        snapshot.progress = BackupProgressSnapshot::failed(&message);
        snapshot.error = Some(message);
        Ok(Some(snapshot.clone()))
    }

    pub fn cancel(
        &self,
        task_id: &str,
        message: impl Into<String>,
    ) -> Result<Option<BackupJobSnapshot>> {
        let message = message.into();
        let mut jobs = self.jobs.write().map_err(lock_error)?;
        let Some(snapshot) = jobs.get_mut(task_id.trim()) else {
            return Ok(None);
        };
        snapshot.progress = BackupProgressSnapshot::cancelled(message);
        snapshot.error = None;
        Ok(Some(snapshot.clone()))
    }
}

#[async_trait]
pub trait BackupProgressReader: Send + Sync {
    async fn progress_snapshot(&self, task_id: &str) -> Result<Option<BackupJobSnapshot>>;

    async fn progress_snapshots(&self) -> Result<Vec<BackupJobSnapshot>>;
}

#[async_trait]
impl BackupProgressReader for BackupJobStore {
    async fn progress_snapshot(&self, task_id: &str) -> Result<Option<BackupJobSnapshot>> {
        self.snapshot(task_id)
    }

    async fn progress_snapshots(&self) -> Result<Vec<BackupJobSnapshot>> {
        self.snapshots()
    }
}

fn normalize_task_id(task_id: impl Into<String>) -> Result<String> {
    let task_id = task_id.into();
    let task_id = task_id.trim();
    if task_id.is_empty() {
        return Err(AstrbotError::Pipeline(
            "backup task id must not be empty".to_string(),
        ));
    }
    Ok(task_id.to_string())
}

fn lock_error(error: std::sync::PoisonError<impl std::fmt::Debug>) -> AstrbotError {
    AstrbotError::Pipeline(format!("backup job store lock: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{BackupJobKind, BackupJobStatus, BackupJobStore, BackupProgressSnapshot};

    #[test]
    fn job_store_exposes_progress_snapshots_without_background_maps() {
        let store = BackupJobStore::new();
        store
            .create(
                "export-1",
                BackupJobKind::Export,
                BackupProgressSnapshot::running("packing", 1, 3, "packing archive"),
            )
            .expect("snapshot should store");

        let snapshot = store
            .snapshot("export-1")
            .expect("snapshot should load")
            .expect("snapshot should exist");
        assert_eq!(snapshot.progress.status, BackupJobStatus::Running);
        assert_eq!(snapshot.progress.stage, "packing");
    }
}
