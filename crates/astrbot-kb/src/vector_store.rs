use std::cmp::Ordering;
use std::sync::{Arc, RwLock};

use astrbot_core::Result;
use astrbot_storage::SqliteJsonStore;
use async_trait::async_trait;

use crate::embedding::EmbeddedKnowledgeChunk;
use crate::types::{ChunkId, DocumentId, KnowledgeBaseId, KnowledgeChunk};

const KB_VECTOR_NAMESPACE: &str = "kb_vector_chunks";

#[derive(Clone, Debug, PartialEq)]
pub struct VectorSearchRequest {
    pub kb_ids: Vec<KnowledgeBaseId>,
    pub query_embedding: Vec<f32>,
    pub top_k: usize,
}

impl VectorSearchRequest {
    pub fn new(kb_ids: Vec<KnowledgeBaseId>, query_embedding: Vec<f32>) -> Self {
        Self {
            kb_ids,
            query_embedding,
            top_k: 20,
        }
    }

    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k.max(1);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorSearchResult {
    pub chunk: KnowledgeChunk,
    pub score: f32,
}

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn upsert_chunks(&self, chunks: Vec<EmbeddedKnowledgeChunk>) -> Result<()>;

    async fn search(&self, request: VectorSearchRequest) -> Result<Vec<VectorSearchResult>>;

    async fn delete_document(&self, doc_id: &DocumentId) -> Result<()>;

    async fn delete_chunk(&self, chunk_id: &ChunkId) -> Result<()>;

    async fn list_chunks(&self, kb_id: &KnowledgeBaseId) -> Result<Vec<KnowledgeChunk>>;

    async fn count_chunks(&self, kb_id: &KnowledgeBaseId) -> Result<usize> {
        Ok(self.list_chunks(kb_id).await?.len())
    }
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryVectorStore {
    chunks: Arc<RwLock<Vec<EmbeddedKnowledgeChunk>>>,
}

#[derive(Clone, Debug)]
pub struct SqliteVectorStore {
    store: SqliteJsonStore,
}

impl SqliteVectorStore {
    pub fn new(store: SqliteJsonStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn upsert_chunks(&self, chunks: Vec<EmbeddedKnowledgeChunk>) -> Result<()> {
        let mut guard = self
            .chunks
            .write()
            .map_err(|_| crate::types::kb_error("vector store lock poisoned"))?;
        for chunk in chunks {
            if let Some(existing) = guard
                .iter_mut()
                .find(|existing| existing.chunk.chunk_id == chunk.chunk.chunk_id)
            {
                *existing = chunk;
            } else {
                guard.push(chunk);
            }
        }
        Ok(())
    }

    async fn search(&self, request: VectorSearchRequest) -> Result<Vec<VectorSearchResult>> {
        let guard = self
            .chunks
            .read()
            .map_err(|_| crate::types::kb_error("vector store lock poisoned"))?;
        let mut results = guard
            .iter()
            .filter(|stored| {
                request.kb_ids.is_empty() || request.kb_ids.contains(&stored.chunk.kb_id)
            })
            .map(|stored| VectorSearchResult {
                chunk: stored.chunk.clone(),
                score: dot_product(&request.query_embedding, &stored.embedding),
            })
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.chunk.chunk_index.cmp(&right.chunk.chunk_index))
        });
        results.truncate(request.top_k);
        Ok(results)
    }

    async fn delete_document(&self, doc_id: &DocumentId) -> Result<()> {
        let mut guard = self
            .chunks
            .write()
            .map_err(|_| crate::types::kb_error("vector store lock poisoned"))?;
        guard.retain(|chunk| &chunk.chunk.doc_id != doc_id);
        Ok(())
    }

    async fn delete_chunk(&self, chunk_id: &ChunkId) -> Result<()> {
        let mut guard = self
            .chunks
            .write()
            .map_err(|_| crate::types::kb_error("vector store lock poisoned"))?;
        guard.retain(|chunk| &chunk.chunk.chunk_id != chunk_id);
        Ok(())
    }

    async fn list_chunks(&self, kb_id: &KnowledgeBaseId) -> Result<Vec<KnowledgeChunk>> {
        let guard = self
            .chunks
            .read()
            .map_err(|_| crate::types::kb_error("vector store lock poisoned"))?;
        Ok(guard
            .iter()
            .filter(|stored| &stored.chunk.kb_id == kb_id)
            .map(|stored| stored.chunk.clone())
            .collect())
    }
}

#[async_trait]
impl VectorStore for SqliteVectorStore {
    async fn upsert_chunks(&self, chunks: Vec<EmbeddedKnowledgeChunk>) -> Result<()> {
        for chunk in chunks {
            self.store
                .put_json(KB_VECTOR_NAMESPACE, chunk.chunk.chunk_id.as_str(), &chunk)?;
        }
        Ok(())
    }

    async fn search(&self, request: VectorSearchRequest) -> Result<Vec<VectorSearchResult>> {
        let mut results = self
            .store
            .list_json::<EmbeddedKnowledgeChunk>(KB_VECTOR_NAMESPACE)?
            .into_iter()
            .filter(|stored| {
                request.kb_ids.is_empty() || request.kb_ids.contains(&stored.chunk.kb_id)
            })
            .map(|stored| VectorSearchResult {
                score: dot_product(&request.query_embedding, &stored.embedding),
                chunk: stored.chunk,
            })
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.chunk.chunk_index.cmp(&right.chunk.chunk_index))
        });
        results.truncate(request.top_k);
        Ok(results)
    }

    async fn delete_document(&self, doc_id: &DocumentId) -> Result<()> {
        for stored in self
            .store
            .list_json::<EmbeddedKnowledgeChunk>(KB_VECTOR_NAMESPACE)?
            .into_iter()
            .filter(|stored| &stored.chunk.doc_id == doc_id)
        {
            self.store
                .delete_json(KB_VECTOR_NAMESPACE, stored.chunk.chunk_id.as_str())?;
        }
        Ok(())
    }

    async fn delete_chunk(&self, chunk_id: &ChunkId) -> Result<()> {
        self.store
            .delete_json(KB_VECTOR_NAMESPACE, chunk_id.as_str())?;
        Ok(())
    }

    async fn list_chunks(&self, kb_id: &KnowledgeBaseId) -> Result<Vec<KnowledgeChunk>> {
        let mut chunks = self
            .store
            .list_json::<EmbeddedKnowledgeChunk>(KB_VECTOR_NAMESPACE)?
            .into_iter()
            .filter(|stored| &stored.chunk.kb_id == kb_id)
            .map(|stored| stored.chunk)
            .collect::<Vec<_>>();
        chunks.sort_by(|left, right| {
            left.doc_id
                .cmp(&right.doc_id)
                .then_with(|| left.chunk_index.cmp(&right.chunk_index))
        });
        Ok(chunks)
    }
}

fn dot_product(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| left * right)
        .sum()
}
