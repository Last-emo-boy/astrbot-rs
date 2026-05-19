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
pub mod qdrant;
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
    KnowledgeDocumentCatalog, KnowledgeDocumentSummary, SqliteKnowledgeBaseManagementStore,
};
pub use media_store::{
    InMemoryKnowledgeMediaStore, KnowledgeMediaStore, KnowledgeMediaWriteRequest,
    KnowledgeMediaWriteResult,
};
pub use parser::{
    DocumentParser, HtmlTextParser, MarkdownParser, MediaItem, ParseResult, PlainTextParser,
    strip_html, strip_markdown,
};
pub use preflight::{
    KnowledgeEmbeddingPreflight, KnowledgeProviderPreflightReport,
    KnowledgeProviderPreflightRequest, KnowledgeProviderPreflightService, KnowledgeRerankPreflight,
};
pub use qdrant::{
    QdrantClient, QdrantDistance, QdrantPoint, QdrantSearchHit, QdrantSearchRequest,
};
pub use rank_fusion::{RankFusionHit, ReciprocalRankFusion};
pub use repository::{
    InMemoryKnowledgeDocumentRepository, KnowledgeDocumentRepository,
    SqliteKnowledgeDocumentRepository,
};
pub use retrieval::{
    HybridKnowledgeRetriever, InMemorySparseRetriever, KnowledgeRetrievalRequest,
    KnowledgeRetrievalResult, KnowledgeRetriever, SparseRetrievalPort, SparseRetrievalRequest,
    SparseRetrievalResult, VectorStoreSparseRetriever,
};
pub use types::{ChunkId, DocumentId, KnowledgeBaseId, KnowledgeChunk, MediaId, kb_error};
pub use upload_task::{
    InMemoryKnowledgeUploadTaskStore, KnowledgeUploadProgress, KnowledgeUploadStage,
    KnowledgeUploadTaskId, KnowledgeUploadTaskKind, KnowledgeUploadTaskResult,
    KnowledgeUploadTaskService, KnowledgeUploadTaskStatus, KnowledgeUploadTaskStore,
    KnowledgeUploadTaskSummary,
};
pub use vector_store::{
    InMemoryVectorStore, SqliteVectorStore, VectorSearchRequest, VectorSearchResult, VectorStore,
};

#[cfg(test)]
mod tests;
