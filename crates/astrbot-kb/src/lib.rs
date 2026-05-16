pub mod chunking;
pub mod document;
pub mod embedding;
pub mod formatter;
pub mod parser;
pub mod rank_fusion;
pub mod retrieval;
pub mod types;
pub mod vector_store;

pub use chunking::{ChunkingOptions, DocumentChunker, RecursiveCharacterChunker};
pub use document::{KnowledgeBaseProfile, KnowledgeBaseStats, KnowledgeDocument, KnowledgeMedia};
pub use embedding::{EmbeddedKnowledgeChunk, embed_chunks};
pub use formatter::{KnowledgeContextFormatter, RetrievalContextFormatter};
pub use parser::{DocumentParser, MediaItem, ParseResult, PlainTextParser};
pub use rank_fusion::{RankFusionHit, ReciprocalRankFusion};
pub use retrieval::{
    HybridKnowledgeRetriever, InMemorySparseRetriever, KnowledgeRetrievalRequest,
    KnowledgeRetrievalResult, KnowledgeRetriever, SparseRetrievalPort, SparseRetrievalRequest,
    SparseRetrievalResult,
};
pub use types::{ChunkId, DocumentId, KnowledgeBaseId, KnowledgeChunk, MediaId, kb_error};
pub use vector_store::{InMemoryVectorStore, VectorSearchRequest, VectorSearchResult, VectorStore};

#[cfg(test)]
mod tests;
