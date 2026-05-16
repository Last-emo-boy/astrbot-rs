use serde::{Deserialize, Serialize};

use crate::types::{DocumentId, KnowledgeBaseId, MediaId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeBaseStats {
    pub doc_count: usize,
    pub chunk_count: usize,
}

impl KnowledgeBaseStats {
    pub fn empty() -> Self {
        Self {
            doc_count: 0,
            chunk_count: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeBaseProfile {
    pub kb_id: KnowledgeBaseId,
    pub name: String,
    pub description: Option<String>,
    pub emoji: Option<String>,
    pub embedding_provider_id: String,
    pub rerank_provider_id: Option<String>,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub top_k_dense: usize,
    pub top_k_sparse: usize,
    pub top_m_final: usize,
    pub stats: KnowledgeBaseStats,
}

impl KnowledgeBaseProfile {
    pub fn new(
        kb_id: KnowledgeBaseId,
        name: impl Into<String>,
        embedding_provider_id: impl Into<String>,
    ) -> Self {
        Self {
            kb_id,
            name: name.into(),
            description: None,
            emoji: Some("📚".to_string()),
            embedding_provider_id: embedding_provider_id.into(),
            rerank_provider_id: None,
            chunk_size: 512,
            chunk_overlap: 50,
            top_k_dense: 50,
            top_k_sparse: 50,
            top_m_final: 5,
            stats: KnowledgeBaseStats::empty(),
        }
    }

    pub fn with_retrieval_limits(
        mut self,
        top_k_dense: usize,
        top_k_sparse: usize,
        top_m_final: usize,
    ) -> Self {
        self.top_k_dense = top_k_dense.max(1);
        self.top_k_sparse = top_k_sparse.max(1);
        self.top_m_final = top_m_final.max(1);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeDocument {
    pub doc_id: DocumentId,
    pub kb_id: KnowledgeBaseId,
    pub name: String,
    pub file_type: String,
    pub file_size: usize,
    pub file_path: Option<String>,
    pub chunk_count: usize,
    pub media_count: usize,
}

impl KnowledgeDocument {
    pub fn new(
        doc_id: DocumentId,
        kb_id: KnowledgeBaseId,
        name: impl Into<String>,
        file_type: impl Into<String>,
    ) -> Self {
        Self {
            doc_id,
            kb_id,
            name: name.into(),
            file_type: file_type.into(),
            file_size: 0,
            file_path: None,
            chunk_count: 0,
            media_count: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeMedia {
    pub media_id: MediaId,
    pub doc_id: DocumentId,
    pub kb_id: KnowledgeBaseId,
    pub media_type: String,
    pub file_name: String,
    pub file_path: Option<String>,
    pub file_size: usize,
    pub mime_type: String,
}
