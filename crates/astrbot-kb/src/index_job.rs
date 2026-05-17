use std::sync::Arc;

use astrbot_core::Result;
use async_trait::async_trait;

use crate::embedding::EmbeddedKnowledgeChunk;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnowledgeIndexStage {
    Parsing,
    Media,
    Chunking,
    Embedding,
    VectorUpsert,
    Metadata,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeIndexProgress {
    pub stage: KnowledgeIndexStage,
    pub current: usize,
    pub total: usize,
}

impl KnowledgeIndexProgress {
    pub fn new(stage: KnowledgeIndexStage, current: usize, total: usize) -> Self {
        Self {
            stage,
            current,
            total,
        }
    }
}

#[async_trait]
pub trait KnowledgeIndexProgressSink: Send + Sync {
    async fn record_progress(&self, progress: KnowledgeIndexProgress) -> Result<()>;
}

#[derive(Clone, Debug, Default)]
pub struct NoopKnowledgeIndexProgressSink;

#[async_trait]
impl KnowledgeIndexProgressSink for NoopKnowledgeIndexProgressSink {
    async fn record_progress(&self, _progress: KnowledgeIndexProgress) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct RecordingKnowledgeIndexProgressSink {
    events: Arc<std::sync::RwLock<Vec<KnowledgeIndexProgress>>>,
}

impl RecordingKnowledgeIndexProgressSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<KnowledgeIndexProgress> {
        self.events
            .read()
            .expect("knowledge index progress lock")
            .clone()
    }
}

#[async_trait]
impl KnowledgeIndexProgressSink for RecordingKnowledgeIndexProgressSink {
    async fn record_progress(&self, progress: KnowledgeIndexProgress) -> Result<()> {
        self.events
            .write()
            .expect("knowledge index progress lock")
            .push(progress);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeIndexJob {
    pub job_id: String,
    pub document_name: String,
}

impl KnowledgeIndexJob {
    pub fn new(job_id: impl Into<String>, document_name: impl Into<String>) -> Self {
        Self {
            job_id: job_id.into(),
            document_name: document_name.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KnowledgeVectorBatch {
    pub job: KnowledgeIndexJob,
    pub chunks: Vec<EmbeddedKnowledgeChunk>,
}

#[async_trait]
pub trait KnowledgeVectorPersistencePort: Send + Sync {
    async fn persist_vectors(&self, batch: KnowledgeVectorBatch) -> Result<()>;
}

pub struct VectorStorePersistencePort {
    vector_store: Arc<dyn crate::VectorStore>,
}

impl VectorStorePersistencePort {
    pub fn new(vector_store: Arc<dyn crate::VectorStore>) -> Self {
        Self { vector_store }
    }
}

#[async_trait]
impl KnowledgeVectorPersistencePort for VectorStorePersistencePort {
    async fn persist_vectors(&self, batch: KnowledgeVectorBatch) -> Result<()> {
        self.vector_store.upsert_chunks(batch.chunks).await
    }
}
