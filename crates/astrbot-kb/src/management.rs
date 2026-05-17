use std::sync::{Arc, RwLock};

use astrbot_core::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::document::{KnowledgeBaseProfile, KnowledgeBaseStats, KnowledgeDocument};
use crate::types::{ChunkId, DocumentId, KnowledgeBaseId, KnowledgeChunk, kb_error};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeBaseCreateCommand {
    pub kb_id: KnowledgeBaseId,
    pub name: String,
    pub description: Option<String>,
    pub emoji: Option<String>,
    pub embedding_provider_id: String,
    pub rerank_provider_id: Option<String>,
    pub chunk_size: Option<usize>,
    pub chunk_overlap: Option<usize>,
    pub top_k_dense: Option<usize>,
    pub top_k_sparse: Option<usize>,
    pub top_m_final: Option<usize>,
}

impl KnowledgeBaseCreateCommand {
    pub fn new(
        kb_id: KnowledgeBaseId,
        name: impl Into<String>,
        embedding_provider_id: impl Into<String>,
    ) -> Self {
        Self {
            kb_id,
            name: name.into(),
            description: None,
            emoji: None,
            embedding_provider_id: embedding_provider_id.into(),
            rerank_provider_id: None,
            chunk_size: None,
            chunk_overlap: None,
            top_k_dense: None,
            top_k_sparse: None,
            top_m_final: None,
        }
    }

    pub fn with_description(mut self, description: Option<String>) -> Self {
        self.description = non_empty_option(description);
        self
    }

    pub fn with_emoji(mut self, emoji: Option<String>) -> Self {
        self.emoji = non_empty_option(emoji);
        self
    }

    pub fn with_rerank_provider_id(mut self, rerank_provider_id: Option<String>) -> Self {
        self.rerank_provider_id = non_empty_option(rerank_provider_id);
        self
    }

    pub fn with_chunking(
        mut self,
        chunk_size: Option<usize>,
        chunk_overlap: Option<usize>,
    ) -> Self {
        self.chunk_size = chunk_size;
        self.chunk_overlap = chunk_overlap;
        self
    }

    pub fn with_retrieval_limits(
        mut self,
        top_k_dense: Option<usize>,
        top_k_sparse: Option<usize>,
        top_m_final: Option<usize>,
    ) -> Self {
        self.top_k_dense = top_k_dense;
        self.top_k_sparse = top_k_sparse;
        self.top_m_final = top_m_final;
        self
    }

    fn into_profile(self) -> Result<KnowledgeBaseProfile> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(kb_error("knowledge base name cannot be empty"));
        }
        let embedding_provider_id = self.embedding_provider_id.trim();
        if embedding_provider_id.is_empty() {
            return Err(kb_error("embedding provider id cannot be empty"));
        }

        let mut profile =
            KnowledgeBaseProfile::new(self.kb_id, name, embedding_provider_id.to_string());
        profile.description = self.description;
        if self.emoji.is_some() {
            profile.emoji = self.emoji;
        }
        profile.rerank_provider_id = self.rerank_provider_id;
        if let Some(chunk_size) = self.chunk_size {
            profile.chunk_size = chunk_size.max(1);
        }
        if let Some(chunk_overlap) = self.chunk_overlap {
            profile.chunk_overlap = chunk_overlap;
        }
        let top_k_dense = self.top_k_dense.unwrap_or(profile.top_k_dense);
        let top_k_sparse = self.top_k_sparse.unwrap_or(profile.top_k_sparse);
        let top_m_final = self.top_m_final.unwrap_or(profile.top_m_final);
        profile = profile.with_retrieval_limits(top_k_dense, top_k_sparse, top_m_final);
        Ok(profile)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KnowledgeBaseUpdateCommand {
    pub name: Option<String>,
    pub description: Option<String>,
    pub emoji: Option<String>,
    pub embedding_provider_id: Option<String>,
    pub rerank_provider_id: Option<String>,
    pub chunk_size: Option<usize>,
    pub chunk_overlap: Option<usize>,
    pub top_k_dense: Option<usize>,
    pub top_k_sparse: Option<usize>,
    pub top_m_final: Option<usize>,
}

impl KnowledgeBaseUpdateCommand {
    fn apply_to(self, profile: &mut KnowledgeBaseProfile) -> Result<()> {
        if let Some(name) = non_empty_option(self.name) {
            profile.name = name;
        }
        if let Some(description) = self.description {
            profile.description = non_empty_option(Some(description));
        }
        if let Some(emoji) = self.emoji {
            profile.emoji = non_empty_option(Some(emoji));
        }
        if let Some(embedding_provider_id) = non_empty_option(self.embedding_provider_id) {
            profile.embedding_provider_id = embedding_provider_id;
        }
        if let Some(rerank_provider_id) = self.rerank_provider_id {
            profile.rerank_provider_id = non_empty_option(Some(rerank_provider_id));
        }
        if let Some(chunk_size) = self.chunk_size {
            profile.chunk_size = chunk_size.max(1);
        }
        if let Some(chunk_overlap) = self.chunk_overlap {
            profile.chunk_overlap = chunk_overlap;
        }
        profile.top_k_dense = self.top_k_dense.unwrap_or(profile.top_k_dense).max(1);
        profile.top_k_sparse = self.top_k_sparse.unwrap_or(profile.top_k_sparse).max(1);
        profile.top_m_final = self.top_m_final.unwrap_or(profile.top_m_final).max(1);
        if profile.chunk_overlap >= profile.chunk_size {
            return Err(kb_error("chunk overlap must be smaller than chunk size"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeBaseSummary {
    pub kb_id: String,
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

impl KnowledgeBaseSummary {
    pub fn from_profile(profile: KnowledgeBaseProfile, stats: KnowledgeBaseStats) -> Self {
        Self {
            kb_id: profile.kb_id.to_string(),
            name: profile.name,
            description: profile.description,
            emoji: profile.emoji,
            embedding_provider_id: profile.embedding_provider_id,
            rerank_provider_id: profile.rerank_provider_id,
            chunk_size: profile.chunk_size,
            chunk_overlap: profile.chunk_overlap,
            top_k_dense: profile.top_k_dense,
            top_k_sparse: profile.top_k_sparse,
            top_m_final: profile.top_m_final,
            stats,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeDocumentSummary {
    pub doc_id: String,
    pub kb_id: String,
    pub name: String,
    pub file_type: String,
    pub file_size: usize,
    pub file_path: Option<String>,
    pub chunk_count: usize,
    pub media_count: usize,
}

impl From<KnowledgeDocument> for KnowledgeDocumentSummary {
    fn from(document: KnowledgeDocument) -> Self {
        Self {
            doc_id: document.doc_id.to_string(),
            kb_id: document.kb_id.to_string(),
            name: document.name,
            file_type: document.file_type,
            file_size: document.file_size,
            file_path: document.file_path,
            chunk_count: document.chunk_count,
            media_count: document.media_count,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeChunkSummary {
    pub chunk_id: String,
    pub doc_id: String,
    pub kb_id: String,
    pub chunk_index: usize,
    pub content: String,
    pub char_count: usize,
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

impl From<KnowledgeChunk> for KnowledgeChunkSummary {
    fn from(chunk: KnowledgeChunk) -> Self {
        Self {
            chunk_id: chunk.chunk_id.to_string(),
            doc_id: chunk.doc_id.to_string(),
            kb_id: chunk.kb_id.to_string(),
            chunk_index: chunk.chunk_index,
            char_count: chunk.char_count(),
            content: chunk.content,
            metadata: chunk.metadata,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeBaseCatalog {
    pub knowledge_bases: Vec<KnowledgeBaseSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeDocumentCatalog {
    pub documents: Vec<KnowledgeDocumentSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeChunkCatalog {
    pub chunks: Vec<KnowledgeChunkSummary>,
}

#[async_trait]
pub trait KnowledgeBaseManagementStore: Send + Sync {
    async fn upsert_kb(&self, profile: KnowledgeBaseProfile) -> Result<()>;

    async fn get_kb(&self, kb_id: &KnowledgeBaseId) -> Result<Option<KnowledgeBaseProfile>>;

    async fn list_kbs(&self) -> Result<Vec<KnowledgeBaseProfile>>;

    async fn delete_kb(&self, kb_id: &KnowledgeBaseId) -> Result<bool>;

    async fn upsert_document(&self, document: KnowledgeDocument) -> Result<()>;

    async fn get_document(&self, doc_id: &DocumentId) -> Result<Option<KnowledgeDocument>>;

    async fn list_documents(&self, kb_id: &KnowledgeBaseId) -> Result<Vec<KnowledgeDocument>>;

    async fn delete_document(&self, doc_id: &DocumentId) -> Result<bool>;

    async fn upsert_chunk(&self, chunk: KnowledgeChunk) -> Result<()>;

    async fn list_chunks_for_kb(&self, kb_id: &KnowledgeBaseId) -> Result<Vec<KnowledgeChunk>>;

    async fn list_chunks_for_document(&self, doc_id: &DocumentId) -> Result<Vec<KnowledgeChunk>>;

    async fn delete_chunk(&self, chunk_id: &ChunkId) -> Result<bool>;
}

#[derive(Clone)]
pub struct KnowledgeBaseManagementService {
    store: Arc<dyn KnowledgeBaseManagementStore>,
}

impl std::fmt::Debug for KnowledgeBaseManagementService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KnowledgeBaseManagementService")
            .finish_non_exhaustive()
    }
}

impl KnowledgeBaseManagementService {
    pub fn new(store: Arc<dyn KnowledgeBaseManagementStore>) -> Self {
        Self { store }
    }

    pub async fn create_kb(
        &self,
        command: KnowledgeBaseCreateCommand,
    ) -> Result<KnowledgeBaseSummary> {
        let profile = command.into_profile()?;
        self.store.upsert_kb(profile.clone()).await?;
        Ok(KnowledgeBaseSummary::from_profile(
            profile,
            KnowledgeBaseStats::empty(),
        ))
    }

    pub async fn list_kbs(&self) -> Result<KnowledgeBaseCatalog> {
        let profiles = self.store.list_kbs().await?;
        let mut knowledge_bases = Vec::with_capacity(profiles.len());
        for profile in profiles {
            let stats = self.stats_for(&profile.kb_id).await?;
            knowledge_bases.push(KnowledgeBaseSummary::from_profile(profile, stats));
        }
        Ok(KnowledgeBaseCatalog { knowledge_bases })
    }

    pub async fn get_kb(&self, kb_id: &KnowledgeBaseId) -> Result<Option<KnowledgeBaseSummary>> {
        let Some(profile) = self.store.get_kb(kb_id).await? else {
            return Ok(None);
        };
        let stats = self.stats_for(kb_id).await?;
        Ok(Some(KnowledgeBaseSummary::from_profile(profile, stats)))
    }

    pub async fn update_kb(
        &self,
        kb_id: &KnowledgeBaseId,
        command: KnowledgeBaseUpdateCommand,
    ) -> Result<Option<KnowledgeBaseSummary>> {
        let Some(mut profile) = self.store.get_kb(kb_id).await? else {
            return Ok(None);
        };
        command.apply_to(&mut profile)?;
        self.store.upsert_kb(profile.clone()).await?;
        let stats = self.stats_for(kb_id).await?;
        Ok(Some(KnowledgeBaseSummary::from_profile(profile, stats)))
    }

    pub async fn delete_kb(&self, kb_id: &KnowledgeBaseId) -> Result<bool> {
        self.store.delete_kb(kb_id).await
    }

    pub async fn stats_for(&self, kb_id: &KnowledgeBaseId) -> Result<KnowledgeBaseStats> {
        let documents = self.store.list_documents(kb_id).await?;
        let chunks = self.store.list_chunks_for_kb(kb_id).await?;
        Ok(KnowledgeBaseStats {
            doc_count: documents.len(),
            chunk_count: chunks.len(),
        })
    }

    pub async fn upsert_document(&self, document: KnowledgeDocument) -> Result<()> {
        self.store.upsert_document(document).await
    }

    pub async fn get_document(
        &self,
        doc_id: &DocumentId,
    ) -> Result<Option<KnowledgeDocumentSummary>> {
        Ok(self.store.get_document(doc_id).await?.map(Into::into))
    }

    pub async fn list_documents(
        &self,
        kb_id: &KnowledgeBaseId,
    ) -> Result<KnowledgeDocumentCatalog> {
        Ok(KnowledgeDocumentCatalog {
            documents: self
                .store
                .list_documents(kb_id)
                .await?
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }

    pub async fn delete_document(&self, doc_id: &DocumentId) -> Result<bool> {
        self.store.delete_document(doc_id).await
    }

    pub async fn upsert_chunk(&self, chunk: KnowledgeChunk) -> Result<()> {
        self.store.upsert_chunk(chunk).await
    }

    pub async fn list_chunks_for_document(
        &self,
        doc_id: &DocumentId,
    ) -> Result<KnowledgeChunkCatalog> {
        Ok(KnowledgeChunkCatalog {
            chunks: self
                .store
                .list_chunks_for_document(doc_id)
                .await?
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }

    pub async fn delete_chunk(&self, chunk_id: &ChunkId) -> Result<bool> {
        self.store.delete_chunk(chunk_id).await
    }
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryKnowledgeBaseManagementStore {
    profiles: Arc<RwLock<Vec<KnowledgeBaseProfile>>>,
    documents: Arc<RwLock<Vec<KnowledgeDocument>>>,
    chunks: Arc<RwLock<Vec<KnowledgeChunk>>>,
}

impl InMemoryKnowledgeBaseManagementStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl KnowledgeBaseManagementStore for InMemoryKnowledgeBaseManagementStore {
    async fn upsert_kb(&self, profile: KnowledgeBaseProfile) -> Result<()> {
        let mut profiles = self
            .profiles
            .write()
            .map_err(|_| kb_error("knowledge management profile lock poisoned"))?;
        if let Some(existing) = profiles
            .iter_mut()
            .find(|existing| existing.kb_id == profile.kb_id)
        {
            *existing = profile;
        } else {
            profiles.push(profile);
        }
        Ok(())
    }

    async fn get_kb(&self, kb_id: &KnowledgeBaseId) -> Result<Option<KnowledgeBaseProfile>> {
        let profiles = self
            .profiles
            .read()
            .map_err(|_| kb_error("knowledge management profile lock poisoned"))?;
        Ok(profiles
            .iter()
            .find(|profile| &profile.kb_id == kb_id)
            .cloned())
    }

    async fn list_kbs(&self) -> Result<Vec<KnowledgeBaseProfile>> {
        let mut profiles = self
            .profiles
            .read()
            .map_err(|_| kb_error("knowledge management profile lock poisoned"))?
            .clone();
        profiles.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(profiles)
    }

    async fn delete_kb(&self, kb_id: &KnowledgeBaseId) -> Result<bool> {
        let mut profiles = self
            .profiles
            .write()
            .map_err(|_| kb_error("knowledge management profile lock poisoned"))?;
        let before = profiles.len();
        profiles.retain(|profile| &profile.kb_id != kb_id);
        drop(profiles);

        self.documents
            .write()
            .map_err(|_| kb_error("knowledge management document lock poisoned"))?
            .retain(|document| &document.kb_id != kb_id);
        self.chunks
            .write()
            .map_err(|_| kb_error("knowledge management chunk lock poisoned"))?
            .retain(|chunk| &chunk.kb_id != kb_id);
        Ok(before
            != self
                .profiles
                .read()
                .map_err(|_| kb_error("knowledge management profile lock poisoned"))?
                .len())
    }

    async fn upsert_document(&self, document: KnowledgeDocument) -> Result<()> {
        let mut documents = self
            .documents
            .write()
            .map_err(|_| kb_error("knowledge management document lock poisoned"))?;
        if let Some(existing) = documents
            .iter_mut()
            .find(|existing| existing.doc_id == document.doc_id)
        {
            *existing = document;
        } else {
            documents.push(document);
        }
        Ok(())
    }

    async fn get_document(&self, doc_id: &DocumentId) -> Result<Option<KnowledgeDocument>> {
        let documents = self
            .documents
            .read()
            .map_err(|_| kb_error("knowledge management document lock poisoned"))?;
        Ok(documents
            .iter()
            .find(|document| &document.doc_id == doc_id)
            .cloned())
    }

    async fn list_documents(&self, kb_id: &KnowledgeBaseId) -> Result<Vec<KnowledgeDocument>> {
        let mut documents = self
            .documents
            .read()
            .map_err(|_| kb_error("knowledge management document lock poisoned"))?
            .iter()
            .filter(|document| &document.kb_id == kb_id)
            .cloned()
            .collect::<Vec<_>>();
        documents.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(documents)
    }

    async fn delete_document(&self, doc_id: &DocumentId) -> Result<bool> {
        let mut documents = self
            .documents
            .write()
            .map_err(|_| kb_error("knowledge management document lock poisoned"))?;
        let before = documents.len();
        documents.retain(|document| &document.doc_id != doc_id);
        let deleted = before != documents.len();
        drop(documents);
        self.chunks
            .write()
            .map_err(|_| kb_error("knowledge management chunk lock poisoned"))?
            .retain(|chunk| &chunk.doc_id != doc_id);
        Ok(deleted)
    }

    async fn upsert_chunk(&self, chunk: KnowledgeChunk) -> Result<()> {
        let mut chunks = self
            .chunks
            .write()
            .map_err(|_| kb_error("knowledge management chunk lock poisoned"))?;
        if let Some(existing) = chunks
            .iter_mut()
            .find(|existing| existing.chunk_id == chunk.chunk_id)
        {
            *existing = chunk;
        } else {
            chunks.push(chunk);
        }
        Ok(())
    }

    async fn list_chunks_for_kb(&self, kb_id: &KnowledgeBaseId) -> Result<Vec<KnowledgeChunk>> {
        let mut chunks = self
            .chunks
            .read()
            .map_err(|_| kb_error("knowledge management chunk lock poisoned"))?
            .iter()
            .filter(|chunk| &chunk.kb_id == kb_id)
            .cloned()
            .collect::<Vec<_>>();
        chunks.sort_by(|left, right| {
            left.doc_id
                .cmp(&right.doc_id)
                .then_with(|| left.chunk_index.cmp(&right.chunk_index))
        });
        Ok(chunks)
    }

    async fn list_chunks_for_document(&self, doc_id: &DocumentId) -> Result<Vec<KnowledgeChunk>> {
        let mut chunks = self
            .chunks
            .read()
            .map_err(|_| kb_error("knowledge management chunk lock poisoned"))?
            .iter()
            .filter(|chunk| &chunk.doc_id == doc_id)
            .cloned()
            .collect::<Vec<_>>();
        chunks.sort_by(|left, right| left.chunk_index.cmp(&right.chunk_index));
        Ok(chunks)
    }

    async fn delete_chunk(&self, chunk_id: &ChunkId) -> Result<bool> {
        let mut chunks = self
            .chunks
            .write()
            .map_err(|_| kb_error("knowledge management chunk lock poisoned"))?;
        let before = chunks.len();
        chunks.retain(|chunk| &chunk.chunk_id != chunk_id);
        Ok(before != chunks.len())
    }
}

fn non_empty_option(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}
