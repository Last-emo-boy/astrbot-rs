use std::sync::{Arc, RwLock};

use astrbot_core::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::types::kb_error;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KnowledgeUploadTaskId(String);

impl KnowledgeUploadTaskId {
    pub fn new(id: impl Into<String>) -> Result<Self> {
        let id = id.into().trim().to_string();
        if id.is_empty() {
            return Err(kb_error("knowledge upload task id cannot be empty"));
        }
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for KnowledgeUploadTaskId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeUploadTaskKind {
    Upload,
    Import,
    Url,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeUploadTaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeUploadStage {
    Queued,
    Parsing,
    Extracting,
    Cleaning,
    Chunking,
    Embedding,
    Metadata,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeUploadProgress {
    pub status: KnowledgeUploadTaskStatus,
    pub file_index: usize,
    pub file_total: usize,
    pub file_name: Option<String>,
    pub stage: KnowledgeUploadStage,
    pub current: usize,
    pub total: usize,
}

impl KnowledgeUploadProgress {
    pub fn queued(file_total: usize) -> Self {
        Self {
            status: KnowledgeUploadTaskStatus::Pending,
            file_index: 0,
            file_total,
            file_name: None,
            stage: KnowledgeUploadStage::Queued,
            current: 0,
            total: file_total,
        }
    }

    pub fn processing(
        mut self,
        file_index: usize,
        file_name: impl Into<String>,
        stage: KnowledgeUploadStage,
        current: usize,
        total: usize,
    ) -> Self {
        self.status = KnowledgeUploadTaskStatus::Processing;
        self.file_index = file_index;
        self.file_name = Some(file_name.into());
        self.stage = stage;
        self.current = current;
        self.total = total.max(1);
        self
    }

    pub fn complete(mut self) -> Self {
        self.status = KnowledgeUploadTaskStatus::Completed;
        self.stage = KnowledgeUploadStage::Completed;
        self.current = self.total.max(self.current);
        self
    }

    pub fn fail(mut self) -> Self {
        self.status = KnowledgeUploadTaskStatus::Failed;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeUploadTaskResult {
    pub document_ids: Vec<String>,
    pub doc_count: usize,
    pub chunk_count: usize,
}

impl KnowledgeUploadTaskResult {
    pub fn new(document_ids: Vec<String>, chunk_count: usize) -> Self {
        Self {
            doc_count: document_ids.len(),
            document_ids,
            chunk_count,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeUploadTaskSummary {
    pub task_id: KnowledgeUploadTaskId,
    pub kind: KnowledgeUploadTaskKind,
    pub kb_id: String,
    pub status: KnowledgeUploadTaskStatus,
    pub progress: Option<KnowledgeUploadProgress>,
    pub result: Option<KnowledgeUploadTaskResult>,
    pub error: Option<String>,
}

impl KnowledgeUploadTaskSummary {
    pub fn new(
        task_id: KnowledgeUploadTaskId,
        kind: KnowledgeUploadTaskKind,
        kb_id: impl Into<String>,
    ) -> Self {
        Self {
            task_id,
            kind,
            kb_id: kb_id.into(),
            status: KnowledgeUploadTaskStatus::Pending,
            progress: None,
            result: None,
            error: None,
        }
    }

    pub fn with_progress(mut self, progress: KnowledgeUploadProgress) -> Self {
        self.status = progress.status;
        self.progress = Some(progress);
        self
    }

    pub fn completed(mut self, result: KnowledgeUploadTaskResult) -> Self {
        self.status = KnowledgeUploadTaskStatus::Completed;
        self.result = Some(result);
        self.error = None;
        if let Some(progress) = self.progress.take() {
            self.progress = Some(progress.complete());
        }
        self
    }

    pub fn failed(mut self, error: impl Into<String>) -> Self {
        self.status = KnowledgeUploadTaskStatus::Failed;
        self.error = Some(error.into());
        if let Some(progress) = self.progress.take() {
            self.progress = Some(progress.fail());
        }
        self
    }
}

#[async_trait]
pub trait KnowledgeUploadTaskStore: Send + Sync {
    async fn put_task(&self, task: KnowledgeUploadTaskSummary) -> Result<()>;

    async fn get_task(
        &self,
        task_id: &KnowledgeUploadTaskId,
    ) -> Result<Option<KnowledgeUploadTaskSummary>>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryKnowledgeUploadTaskStore {
    tasks: Arc<RwLock<Vec<KnowledgeUploadTaskSummary>>>,
}

impl InMemoryKnowledgeUploadTaskStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl KnowledgeUploadTaskStore for InMemoryKnowledgeUploadTaskStore {
    async fn put_task(&self, task: KnowledgeUploadTaskSummary) -> Result<()> {
        let mut tasks = self
            .tasks
            .write()
            .map_err(|_| kb_error("knowledge upload task lock poisoned"))?;
        if let Some(existing) = tasks
            .iter_mut()
            .find(|existing| existing.task_id == task.task_id)
        {
            *existing = task;
        } else {
            tasks.push(task);
        }
        Ok(())
    }

    async fn get_task(
        &self,
        task_id: &KnowledgeUploadTaskId,
    ) -> Result<Option<KnowledgeUploadTaskSummary>> {
        let tasks = self
            .tasks
            .read()
            .map_err(|_| kb_error("knowledge upload task lock poisoned"))?;
        Ok(tasks.iter().find(|task| &task.task_id == task_id).cloned())
    }
}

#[derive(Clone)]
pub struct KnowledgeUploadTaskService {
    store: Arc<dyn KnowledgeUploadTaskStore>,
}

impl std::fmt::Debug for KnowledgeUploadTaskService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KnowledgeUploadTaskService")
            .finish_non_exhaustive()
    }
}

impl KnowledgeUploadTaskService {
    pub fn new(store: Arc<dyn KnowledgeUploadTaskStore>) -> Self {
        Self { store }
    }

    pub async fn start_task(
        &self,
        task_id: KnowledgeUploadTaskId,
        kind: KnowledgeUploadTaskKind,
        kb_id: impl Into<String>,
        file_total: usize,
    ) -> Result<KnowledgeUploadTaskSummary> {
        let summary = KnowledgeUploadTaskSummary::new(task_id, kind, kb_id)
            .with_progress(KnowledgeUploadProgress::queued(file_total));
        self.store.put_task(summary.clone()).await?;
        Ok(summary)
    }

    pub async fn update_progress(
        &self,
        task_id: &KnowledgeUploadTaskId,
        progress: KnowledgeUploadProgress,
    ) -> Result<Option<KnowledgeUploadTaskSummary>> {
        let Some(summary) = self.store.get_task(task_id).await? else {
            return Ok(None);
        };
        let summary = summary.with_progress(progress);
        self.store.put_task(summary.clone()).await?;
        Ok(Some(summary))
    }

    pub async fn complete_task(
        &self,
        task_id: &KnowledgeUploadTaskId,
        result: KnowledgeUploadTaskResult,
    ) -> Result<Option<KnowledgeUploadTaskSummary>> {
        let Some(summary) = self.store.get_task(task_id).await? else {
            return Ok(None);
        };
        let summary = summary.completed(result);
        self.store.put_task(summary.clone()).await?;
        Ok(Some(summary))
    }

    pub async fn fail_task(
        &self,
        task_id: &KnowledgeUploadTaskId,
        error: impl Into<String>,
    ) -> Result<Option<KnowledgeUploadTaskSummary>> {
        let Some(summary) = self.store.get_task(task_id).await? else {
            return Ok(None);
        };
        let summary = summary.failed(error);
        self.store.put_task(summary.clone()).await?;
        Ok(Some(summary))
    }

    pub async fn task(
        &self,
        task_id: &KnowledgeUploadTaskId,
    ) -> Result<Option<KnowledgeUploadTaskSummary>> {
        self.store.get_task(task_id).await
    }
}
