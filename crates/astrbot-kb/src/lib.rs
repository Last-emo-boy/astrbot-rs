pub mod chunking;
pub mod document;
pub mod embedding;
pub mod formatter;
pub mod index_job;
pub mod ingestion;
pub mod management;
pub mod media_store;
pub mod parser;
pub mod preflight;
pub mod rank_fusion;
pub mod repository;
pub mod retrieval;
pub mod types;
pub mod upload_task;
pub mod vector_store;

pub use chunking::{ChunkingOptions, DocumentChunker, RecursiveCharacterChunker};
pub use document::{KnowledgeBaseProfile, KnowledgeBaseStats, KnowledgeDocument, KnowledgeMedia};
pub use embedding::{EmbeddedKnowledgeChunk, embed_chunks};
pub use formatter::{KnowledgeContextFormatter, RetrievalContextFormatter};
pub use index_job::{
    KnowledgeIndexJob, KnowledgeIndexProgress, KnowledgeIndexProgressSink, KnowledgeIndexStage,
    KnowledgeVectorBatch, KnowledgeVectorPersistencePort, NoopKnowledgeIndexProgressSink,
    RecordingKnowledgeIndexProgressSink, VectorStorePersistencePort,
};
pub use ingestion::{
    KnowledgeIngestionOutcome, KnowledgeIngestionRequest, KnowledgeIngestionService,
};
pub use management::{
    InMemoryKnowledgeBaseManagementStore, KnowledgeBaseCatalog, KnowledgeBaseCreateCommand,
    KnowledgeBaseManagementService, KnowledgeBaseManagementStore, KnowledgeBaseSummary,
    KnowledgeBaseUpdateCommand, KnowledgeChunkCatalog, KnowledgeChunkSummary,
    KnowledgeDocumentCatalog, KnowledgeDocumentSummary,
};
pub use media_store::{
    InMemoryKnowledgeMediaStore, KnowledgeMediaStore, KnowledgeMediaWriteRequest,
    KnowledgeMediaWriteResult,
};
pub use parser::{DocumentParser, MediaItem, ParseResult, PlainTextParser};
pub use preflight::{
    KnowledgeEmbeddingPreflight, KnowledgeProviderPreflightReport,
    KnowledgeProviderPreflightRequest, KnowledgeProviderPreflightService, KnowledgeRerankPreflight,
};
pub use rank_fusion::{RankFusionHit, ReciprocalRankFusion};
pub use repository::{InMemoryKnowledgeDocumentRepository, KnowledgeDocumentRepository};
pub use retrieval::{
    HybridKnowledgeRetriever, InMemorySparseRetriever, KnowledgeRetrievalRequest,
    KnowledgeRetrievalResult, KnowledgeRetriever, SparseRetrievalPort, SparseRetrievalRequest,
    SparseRetrievalResult,
};
pub use types::{ChunkId, DocumentId, KnowledgeBaseId, KnowledgeChunk, MediaId, kb_error};
pub use upload_task::{
    InMemoryKnowledgeUploadTaskStore, KnowledgeUploadProgress, KnowledgeUploadStage,
    KnowledgeUploadTaskId, KnowledgeUploadTaskKind, KnowledgeUploadTaskResult,
    KnowledgeUploadTaskService, KnowledgeUploadTaskStatus, KnowledgeUploadTaskStore,
    KnowledgeUploadTaskSummary,
};
pub use vector_store::{InMemoryVectorStore, VectorSearchRequest, VectorSearchResult, VectorStore};

#[cfg(test)]
mod tests;
