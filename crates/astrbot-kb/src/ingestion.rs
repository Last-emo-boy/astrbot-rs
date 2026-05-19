use std::sync::Arc;

use astrbot_core::Result;
use astrbot_provider::EmbeddingProvider;

use crate::chunking::{ChunkingOptions, DocumentChunker};
use crate::document::{KnowledgeDocument, KnowledgeMedia};
use crate::embedding::embed_chunks;
use crate::index_job::{
    KnowledgeIndexJob, KnowledgeIndexProgress, KnowledgeIndexProgressSink, KnowledgeIndexStage,
    KnowledgeVectorBatch, KnowledgeVectorPersistencePort, NoopKnowledgeIndexProgressSink,
};
use crate::media_store::{KnowledgeMediaStore, KnowledgeMediaWriteRequest};
use crate::parser::DocumentParser;
use crate::repository::KnowledgeDocumentRepository;
use crate::types::{ChunkId, DocumentId, KnowledgeBaseId, KnowledgeChunk, MediaId, kb_error};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeIngestionRequest {
    pub kb_id: KnowledgeBaseId,
    pub doc_id: DocumentId,
    pub file_name: String,
    pub file_type: String,
    pub file_content: Vec<u8>,
    pub embedding_provider_id: Option<String>,
    pub embedding_model: Option<String>,
    pub chunking: ChunkingOptions,
}

impl KnowledgeIngestionRequest {
    pub fn new(
        kb_id: KnowledgeBaseId,
        doc_id: DocumentId,
        file_name: impl Into<String>,
        file_type: impl Into<String>,
        file_content: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            kb_id,
            doc_id,
            file_name: file_name.into(),
            file_type: file_type.into(),
            file_content: file_content.into(),
            embedding_provider_id: None,
            embedding_model: None,
            chunking: ChunkingOptions::default(),
        }
    }

    pub fn with_embedding_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        let provider_id = provider_id.into();
        let provider_id = provider_id.trim();
        if !provider_id.is_empty() {
            self.embedding_provider_id = Some(provider_id.to_string());
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KnowledgeIngestionOutcome {
    pub document: KnowledgeDocument,
    pub media: Vec<KnowledgeMedia>,
    pub chunks: Vec<KnowledgeChunk>,
    pub chunk_count: usize,
}

pub struct KnowledgeIngestionService {
    parser: Arc<dyn DocumentParser>,
    chunker: Arc<dyn DocumentChunker>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
    vector_persistence: Arc<dyn KnowledgeVectorPersistencePort>,
    repository: Arc<dyn KnowledgeDocumentRepository>,
    media_store: Arc<dyn KnowledgeMediaStore>,
    progress_sink: Arc<dyn KnowledgeIndexProgressSink>,
}

impl KnowledgeIngestionService {
    pub fn new(
        parser: Arc<dyn DocumentParser>,
        chunker: Arc<dyn DocumentChunker>,
        embedding_provider: Arc<dyn EmbeddingProvider>,
        vector_persistence: Arc<dyn KnowledgeVectorPersistencePort>,
        repository: Arc<dyn KnowledgeDocumentRepository>,
        media_store: Arc<dyn KnowledgeMediaStore>,
    ) -> Self {
        Self {
            parser,
            chunker,
            embedding_provider,
            vector_persistence,
            repository,
            media_store,
            progress_sink: Arc::new(NoopKnowledgeIndexProgressSink),
        }
    }

    pub fn with_progress_sink(
        mut self,
        progress_sink: Arc<dyn KnowledgeIndexProgressSink>,
    ) -> Self {
        self.progress_sink = progress_sink;
        self
    }

    pub async fn ingest(
        &self,
        request: KnowledgeIngestionRequest,
    ) -> Result<KnowledgeIngestionOutcome> {
        self.progress(KnowledgeIndexStage::Parsing, 0, 1).await?;
        let parse_result = self
            .parser
            .parse(request.file_content.clone(), &request.file_name)
            .await?;
        self.progress(KnowledgeIndexStage::Parsing, 1, 1).await?;

        let mut media_records = Vec::new();
        let media_total = parse_result.media.len();
        for (index, media_item) in parse_result.media.into_iter().enumerate() {
            let media_id = MediaId::new(format!("{}-media-{index}", request.doc_id))?;
            let write = self
                .media_store
                .write_media(KnowledgeMediaWriteRequest {
                    kb_id: request.kb_id.clone(),
                    doc_id: request.doc_id.clone(),
                    media_id: media_id.clone(),
                    file_name: media_item.file_name.clone(),
                    content: media_item.content,
                    mime_type: media_item.mime_type.clone(),
                })
                .await?;
            media_records.push(KnowledgeMedia {
                media_id,
                doc_id: request.doc_id.clone(),
                kb_id: request.kb_id.clone(),
                media_type: media_item.media_type,
                file_name: media_item.file_name,
                file_path: Some(write.file_path),
                file_size: write.file_size,
                mime_type: media_item.mime_type,
            });
            self.progress(KnowledgeIndexStage::Media, index + 1, media_total)
                .await?;
        }

        self.progress(KnowledgeIndexStage::Chunking, 0, 1).await?;
        let chunk_texts = self
            .chunker
            .chunk(&parse_result.text, request.chunking.clone())
            .await?;
        self.progress(KnowledgeIndexStage::Chunking, 1, 1).await?;
        if chunk_texts.is_empty() {
            return Err(kb_error("parsed document produced no chunks"));
        }

        let chunks = chunk_texts
            .into_iter()
            .enumerate()
            .map(|(index, text)| {
                let char_count = text.chars().count();
                KnowledgeChunk::new(
                    ChunkId::new(format!("{}-chunk-{index}", request.doc_id))?,
                    request.kb_id.clone(),
                    request.doc_id.clone(),
                    index,
                    text,
                )
                .with_metadata("doc_name", serde_json::json!(request.file_name))
                .with_metadata("chunk_index", serde_json::json!(index))
                .with_metadata("char_count", serde_json::json!(char_count))
                .pipe(Ok)
            })
            .collect::<Result<Vec<_>>>()?;

        self.progress(KnowledgeIndexStage::Embedding, 0, chunks.len())
            .await?;
        let embedded = embed_chunks(
            self.embedding_provider.as_ref(),
            chunks,
            request.embedding_provider_id.clone(),
            request.embedding_model.clone(),
        )
        .await?;
        self.progress(
            KnowledgeIndexStage::Embedding,
            embedded.len(),
            embedded.len(),
        )
        .await?;

        self.progress(KnowledgeIndexStage::VectorUpsert, 0, embedded.len())
            .await?;
        self.vector_persistence
            .persist_vectors(KnowledgeVectorBatch {
                job: KnowledgeIndexJob::new(request.doc_id.to_string(), request.file_name.clone()),
                chunks: embedded.clone(),
            })
            .await?;
        let stored_chunks = embedded
            .iter()
            .map(|embedded| embedded.chunk.clone())
            .collect::<Vec<_>>();
        self.progress(
            KnowledgeIndexStage::VectorUpsert,
            embedded.len(),
            embedded.len(),
        )
        .await?;

        let document = KnowledgeDocument {
            doc_id: request.doc_id.clone(),
            kb_id: request.kb_id.clone(),
            name: request.file_name,
            file_type: request.file_type,
            file_size: request.file_content.len(),
            file_path: None,
            chunk_count: embedded.len(),
            media_count: media_records.len(),
        };

        self.progress(KnowledgeIndexStage::Metadata, 0, 1).await?;
        self.repository.upsert_document(document.clone()).await?;
        for media in &media_records {
            self.repository.upsert_media(media.clone()).await?;
        }
        self.progress(KnowledgeIndexStage::Metadata, 1, 1).await?;
        self.progress(KnowledgeIndexStage::Completed, 1, 1).await?;

        Ok(KnowledgeIngestionOutcome {
            document,
            media: media_records,
            chunks: stored_chunks,
            chunk_count: embedded.len(),
        })
    }

    async fn progress(
        &self,
        stage: KnowledgeIndexStage,
        current: usize,
        total: usize,
    ) -> Result<()> {
        self.progress_sink
            .record_progress(KnowledgeIndexProgress::new(stage, current, total))
            .await
    }
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}
