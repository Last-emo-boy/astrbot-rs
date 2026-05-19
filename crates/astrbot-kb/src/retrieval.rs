use std::collections::BTreeMap;
use std::sync::Arc;

use astrbot_core::Result;
use astrbot_provider::{RerankProvider, RerankRequest};
use async_trait::async_trait;

use crate::rank_fusion::{RankFusionHit, ReciprocalRankFusion};
use crate::types::{KnowledgeBaseId, KnowledgeChunk};
use crate::vector_store::{VectorSearchRequest, VectorStore};

#[derive(Clone, Debug, PartialEq)]
pub struct SparseRetrievalRequest {
    pub query: String,
    pub kb_ids: Vec<KnowledgeBaseId>,
    pub top_k: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SparseRetrievalResult {
    pub chunk: KnowledgeChunk,
    pub score: f32,
}

#[async_trait]
pub trait SparseRetrievalPort: Send + Sync {
    async fn retrieve(&self, request: SparseRetrievalRequest)
    -> Result<Vec<SparseRetrievalResult>>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemorySparseRetriever {
    chunks: Vec<KnowledgeChunk>,
}

impl InMemorySparseRetriever {
    pub fn new(chunks: Vec<KnowledgeChunk>) -> Self {
        Self { chunks }
    }
}

#[derive(Clone)]
pub struct VectorStoreSparseRetriever {
    vector_store: Arc<dyn VectorStore>,
}

impl VectorStoreSparseRetriever {
    pub fn new(vector_store: Arc<dyn VectorStore>) -> Self {
        Self { vector_store }
    }
}

#[async_trait]
impl SparseRetrievalPort for InMemorySparseRetriever {
    async fn retrieve(
        &self,
        request: SparseRetrievalRequest,
    ) -> Result<Vec<SparseRetrievalResult>> {
        let query_terms = tokenize(&request.query);
        let mut results = self
            .chunks
            .iter()
            .filter(|chunk| request.kb_ids.is_empty() || request.kb_ids.contains(&chunk.kb_id))
            .map(|chunk| {
                let content_terms = tokenize(&chunk.content);
                let score = query_terms
                    .keys()
                    .filter(|term| content_terms.contains_key(*term))
                    .count() as f32;
                SparseRetrievalResult {
                    chunk: chunk.clone(),
                    score,
                }
            })
            .filter(|result| result.score > 0.0)
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.chunk.chunk_index.cmp(&right.chunk.chunk_index))
        });
        results.truncate(request.top_k.max(1));
        Ok(results)
    }
}

#[async_trait]
impl SparseRetrievalPort for VectorStoreSparseRetriever {
    async fn retrieve(
        &self,
        request: SparseRetrievalRequest,
    ) -> Result<Vec<SparseRetrievalResult>> {
        if request.kb_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut chunks = Vec::new();
        for kb_id in &request.kb_ids {
            chunks.extend(self.vector_store.list_chunks(kb_id).await?);
        }
        InMemorySparseRetriever::new(chunks).retrieve(request).await
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KnowledgeRetrievalRequest {
    pub query: String,
    pub kb_ids: Vec<KnowledgeBaseId>,
    pub query_embedding: Option<Vec<f32>>,
    pub rerank_provider_id: Option<String>,
    pub top_k_dense: usize,
    pub top_k_sparse: usize,
    pub top_k_fusion: usize,
    pub top_m_final: usize,
}

impl KnowledgeRetrievalRequest {
    pub fn new(query: impl Into<String>, kb_ids: Vec<KnowledgeBaseId>) -> Self {
        Self {
            query: query.into(),
            kb_ids,
            query_embedding: None,
            rerank_provider_id: None,
            top_k_dense: 50,
            top_k_sparse: 50,
            top_k_fusion: 20,
            top_m_final: 5,
        }
    }

    pub fn with_query_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.query_embedding = Some(embedding);
        self
    }

    pub fn with_rerank_provider_id(mut self, provider_id: Option<String>) -> Self {
        self.rerank_provider_id = provider_id.and_then(|provider_id| {
            let provider_id = provider_id.trim().to_string();
            (!provider_id.is_empty()).then_some(provider_id)
        });
        self
    }

    pub fn with_limits(
        mut self,
        top_k_dense: usize,
        top_k_sparse: usize,
        top_k_fusion: usize,
        top_m_final: usize,
    ) -> Self {
        self.top_k_dense = top_k_dense.max(1);
        self.top_k_sparse = top_k_sparse.max(1);
        self.top_k_fusion = top_k_fusion.max(1);
        self.top_m_final = top_m_final.max(1);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KnowledgeRetrievalResult {
    pub chunk_id: String,
    pub doc_id: String,
    pub kb_id: String,
    pub kb_name: Option<String>,
    pub doc_name: Option<String>,
    pub chunk_index: usize,
    pub content: String,
    pub score: f32,
    pub metadata: BTreeMap<String, serde_json::Value>,
}

impl KnowledgeRetrievalResult {
    pub fn from_chunk(chunk: KnowledgeChunk, score: f32) -> Self {
        Self {
            chunk_id: chunk.chunk_id.to_string(),
            doc_id: chunk.doc_id.to_string(),
            kb_id: chunk.kb_id.to_string(),
            kb_name: chunk
                .metadata
                .get("kb_name")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned),
            doc_name: chunk
                .metadata
                .get("doc_name")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned),
            chunk_index: chunk.chunk_index,
            content: chunk.content,
            score,
            metadata: chunk.metadata,
        }
    }
}

#[async_trait]
pub trait KnowledgeRetriever: Send + Sync {
    async fn retrieve(
        &self,
        request: KnowledgeRetrievalRequest,
        rerank_provider: Option<Arc<dyn RerankProvider>>,
    ) -> Result<Vec<KnowledgeRetrievalResult>>;
}

pub struct HybridKnowledgeRetriever {
    vector_store: Arc<dyn VectorStore>,
    sparse_retriever: Arc<dyn SparseRetrievalPort>,
    rank_fusion: ReciprocalRankFusion,
}

impl HybridKnowledgeRetriever {
    pub fn new(
        vector_store: Arc<dyn VectorStore>,
        sparse_retriever: Arc<dyn SparseRetrievalPort>,
    ) -> Self {
        Self {
            vector_store,
            sparse_retriever,
            rank_fusion: ReciprocalRankFusion::default(),
        }
    }
}

#[async_trait]
impl KnowledgeRetriever for HybridKnowledgeRetriever {
    async fn retrieve(
        &self,
        request: KnowledgeRetrievalRequest,
        rerank_provider: Option<Arc<dyn RerankProvider>>,
    ) -> Result<Vec<KnowledgeRetrievalResult>> {
        let dense = if let Some(query_embedding) = request.query_embedding.clone() {
            self.vector_store
                .search(
                    VectorSearchRequest::new(request.kb_ids.clone(), query_embedding)
                        .with_top_k(request.top_k_dense),
                )
                .await?
                .into_iter()
                .map(|hit| RankFusionHit::new(hit.chunk, hit.score))
                .collect()
        } else {
            Vec::new()
        };
        let sparse = self
            .sparse_retriever
            .retrieve(SparseRetrievalRequest {
                query: request.query.clone(),
                kb_ids: request.kb_ids.clone(),
                top_k: request.top_k_sparse,
            })
            .await?
            .into_iter()
            .map(|hit| RankFusionHit::new(hit.chunk, hit.score))
            .collect();

        let mut fused = self
            .rank_fusion
            .fuse(dense, sparse, request.top_k_fusion)
            .into_iter()
            .map(|hit| KnowledgeRetrievalResult::from_chunk(hit.chunk, hit.score))
            .collect::<Vec<_>>();

        if let Some(rerank_provider) = rerank_provider
            && !fused.is_empty()
        {
            let documents = fused
                .iter()
                .map(|result| result.content.clone())
                .collect::<Vec<_>>();
            let reranked = rerank_provider
                .rerank(rerank_request_with_provider_id(
                    RerankRequest::new(request.query.clone(), documents)
                        .with_top_n(request.top_m_final),
                    request.rerank_provider_id.clone(),
                ))
                .await?;
            let mut next = Vec::new();
            for score in reranked.results {
                if let Some(result) = fused.get(score.index) {
                    let mut result = result.clone();
                    result.score = score.relevance_score;
                    next.push(result);
                }
            }
            fused = next;
        }

        fused.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.chunk_index.cmp(&right.chunk_index))
        });
        fused.truncate(request.top_m_final);
        Ok(fused)
    }
}

fn rerank_request_with_provider_id(
    request: RerankRequest,
    provider_id: Option<String>,
) -> RerankRequest {
    match provider_id {
        Some(provider_id) => request.with_provider_id(provider_id),
        None => request,
    }
}

fn tokenize(text: &str) -> BTreeMap<String, usize> {
    let mut tokens = BTreeMap::new();
    for token in text
        .split(|ch: char| ch.is_whitespace() || ch.is_ascii_punctuation())
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        *tokens.entry(token.to_lowercase()).or_insert(0) += 1;
    }
    tokens
}
