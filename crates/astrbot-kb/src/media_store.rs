use std::sync::{Arc, RwLock};

use astrbot_core::Result;
use async_trait::async_trait;

use crate::types::{DocumentId, KnowledgeBaseId, MediaId, kb_error};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeMediaWriteRequest {
    pub kb_id: KnowledgeBaseId,
    pub doc_id: DocumentId,
    pub media_id: MediaId,
    pub file_name: String,
    pub content: Vec<u8>,
    pub mime_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeMediaWriteResult {
    pub file_path: String,
    pub file_size: usize,
}

#[async_trait]
pub trait KnowledgeMediaStore: Send + Sync {
    async fn write_media(
        &self,
        request: KnowledgeMediaWriteRequest,
    ) -> Result<KnowledgeMediaWriteResult>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryKnowledgeMediaStore {
    writes: Arc<RwLock<Vec<KnowledgeMediaWriteRequest>>>,
}

impl InMemoryKnowledgeMediaStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn writes(&self) -> Result<Vec<KnowledgeMediaWriteRequest>> {
        self.writes
            .read()
            .map_err(|_| kb_error("knowledge media store lock poisoned"))
            .map(|writes| writes.clone())
    }
}

#[async_trait]
impl KnowledgeMediaStore for InMemoryKnowledgeMediaStore {
    async fn write_media(
        &self,
        request: KnowledgeMediaWriteRequest,
    ) -> Result<KnowledgeMediaWriteResult> {
        let file_path = format!(
            "memory://{}/{}/{}",
            request.kb_id, request.doc_id, request.file_name
        );
        let file_size = request.content.len();
        self.writes
            .write()
            .map_err(|_| kb_error("knowledge media store lock poisoned"))?
            .push(request);
        Ok(KnowledgeMediaWriteResult {
            file_path,
            file_size,
        })
    }
}
