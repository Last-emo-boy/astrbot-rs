use std::sync::Arc;

use astrbot_provider::{MockEmbeddingProvider, MockRerankProvider};
use astrbot_storage::SqliteJsonStore;

use crate::{
    ChunkId, ChunkingOptions, DocumentChunker, DocumentId, EmbeddedKnowledgeChunk,
    HybridKnowledgeRetriever, InMemoryKnowledgeBaseManagementStore,
    InMemoryKnowledgeDocumentRepository, InMemoryKnowledgeMediaStore,
    InMemoryKnowledgeUploadTaskStore, InMemorySparseRetriever, InMemoryVectorStore,
    KnowledgeBaseCreateCommand, KnowledgeBaseId, KnowledgeBaseManagementService,
    KnowledgeBaseProfile, KnowledgeBaseUpdateCommand, KnowledgeChunk, KnowledgeContextFormatter,
    KnowledgeDocument, KnowledgeDocumentRepository, KnowledgeIndexStage, KnowledgeIngestionRequest,
    KnowledgeIngestionService, KnowledgeMedia, KnowledgeProviderPreflightRequest,
    KnowledgeProviderPreflightService, KnowledgeRetrievalRequest, KnowledgeRetriever,
    KnowledgeUploadProgress, KnowledgeUploadStage, KnowledgeUploadTaskId, KnowledgeUploadTaskKind,
    KnowledgeUploadTaskResult, KnowledgeUploadTaskService, KnowledgeUploadTaskStatus, MediaId,
    PlainTextParser, RecordingKnowledgeIndexProgressSink, RecursiveCharacterChunker,
    RetrievalContextFormatter, SqliteKnowledgeBaseManagementStore,
    SqliteKnowledgeDocumentRepository, SqliteVectorStore, VectorStore, VectorStorePersistencePort,
    VectorStoreSparseRetriever, embed_chunks,
};

fn chunk(id: &str, index: usize, content: &str, embedding: Vec<f32>) -> EmbeddedKnowledgeChunk {
    let kb_id = KnowledgeBaseId::new("kb-1").expect("kb id");
    let doc_id = DocumentId::new("doc-1").expect("doc id");
    let chunk = KnowledgeChunk::new(
        ChunkId::new(id).expect("chunk id"),
        kb_id,
        doc_id,
        index,
        content,
    )
    .with_metadata("kb_name", serde_json::json!("docs"))
    .with_metadata("doc_name", serde_json::json!("intro.md"));
    EmbeddedKnowledgeChunk::new(chunk, embedding)
}

#[tokio::test]
async fn recursive_chunker_splits_with_overlap() {
    let chunker = RecursiveCharacterChunker::with_separators([""]);
    let chunks = chunker
        .chunk("abcdefghij", ChunkingOptions::new(4, 1).expect("options"))
        .await
        .expect("chunking should succeed");

    assert_eq!(chunks, vec!["abcd", "defg", "ghij"]);
}

#[tokio::test]
async fn embed_chunks_keeps_embedding_provider_outside_vector_store() {
    let provider = MockEmbeddingProvider::new(vec![0.25, 0.75]);
    let chunks = vec![KnowledgeChunk::new(
        ChunkId::new("chunk-1").expect("chunk id"),
        KnowledgeBaseId::new("kb-1").expect("kb id"),
        DocumentId::new("doc-1").expect("doc id"),
        0,
        "hello",
    )];

    let embedded = embed_chunks(&provider, chunks, Some("embedding-1".to_string()), None)
        .await
        .expect("embedding should succeed");

    assert_eq!(embedded[0].embedding, vec![0.25, 0.75]);
}

#[tokio::test]
async fn hybrid_retriever_fuses_dense_sparse_and_reranks() {
    let store = Arc::new(InMemoryVectorStore::default());
    let first = chunk("chunk-1", 0, "rust plugin sdk", vec![0.9, 0.1]);
    let second = chunk("chunk-2", 1, "python storage adapter", vec![0.2, 0.8]);
    store
        .upsert_chunks(vec![first.clone(), second.clone()])
        .await
        .expect("upsert should succeed");
    let sparse = Arc::new(InMemorySparseRetriever::new(vec![
        first.chunk.clone(),
        second.chunk.clone(),
    ]));
    let retriever = HybridKnowledgeRetriever::new(store, sparse);
    let reranker = Arc::new(MockRerankProvider::new(vec![0.1, 0.9]));

    let results = retriever
        .retrieve(
            KnowledgeRetrievalRequest::new(
                "plugin storage",
                vec![KnowledgeBaseId::new("kb-1").expect("kb id")],
            )
            .with_query_embedding(vec![1.0, 0.0])
            .with_limits(2, 2, 2, 2),
            Some(reranker),
        )
        .await
        .expect("retrieve should succeed");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].content, "python storage adapter");
    assert_eq!(results[0].score, 0.9);
}

#[test]
fn context_formatter_matches_astrbot_knowledge_prompt_shape() {
    let formatter = RetrievalContextFormatter::default();
    let result = crate::KnowledgeRetrievalResult {
        chunk_id: "chunk-1".to_string(),
        doc_id: "doc-1".to_string(),
        kb_id: "kb-1".to_string(),
        kb_name: Some("docs".to_string()),
        doc_name: Some("intro.md".to_string()),
        chunk_index: 0,
        content: "Rust boundary".to_string(),
        score: 0.42,
        metadata: Default::default(),
    };

    let context = formatter.format_context(&[result]);

    assert!(context.contains("【知识 1】"));
    assert!(context.contains("来源: docs / intro.md"));
    assert!(context.contains("相关度: 0.42"));
}

#[tokio::test]
async fn ingestion_service_orchestrates_parse_chunk_embed_vector_and_metadata_ports() {
    let repository = Arc::new(InMemoryKnowledgeDocumentRepository::new());
    let media_store = Arc::new(InMemoryKnowledgeMediaStore::new());
    let vector_store = Arc::new(InMemoryVectorStore::default());
    let progress = Arc::new(RecordingKnowledgeIndexProgressSink::new());
    let service = KnowledgeIngestionService::new(
        Arc::new(PlainTextParser),
        Arc::new(RecursiveCharacterChunker::with_separators([""])),
        Arc::new(MockEmbeddingProvider::new(vec![0.5, 0.5])),
        Arc::new(VectorStorePersistencePort::new(vector_store.clone())),
        repository.clone(),
        media_store,
    )
    .with_progress_sink(progress.clone());
    let kb_id = KnowledgeBaseId::new("kb-1").expect("kb id");
    let doc_id = DocumentId::new("doc-1").expect("doc id");

    let outcome = service
        .ingest(
            KnowledgeIngestionRequest::new(
                kb_id.clone(),
                doc_id.clone(),
                "intro.txt",
                "txt",
                "abcdefghij",
            )
            .with_embedding_provider_id("embedding-1"),
        )
        .await
        .expect("ingestion should succeed");

    assert_eq!(outcome.document.name, "intro.txt");
    assert_eq!(outcome.chunk_count, 1);
    assert_eq!(
        repository
            .list_documents(&kb_id)
            .await
            .expect("documents should list")
            .len(),
        1
    );
    assert_eq!(
        vector_store
            .count_chunks(&kb_id)
            .await
            .expect("vectors should persist"),
        1
    );
    assert!(
        progress
            .events()
            .iter()
            .any(|event| event.stage == KnowledgeIndexStage::VectorUpsert)
    );
    assert!(
        progress
            .events()
            .iter()
            .any(|event| event.stage == KnowledgeIndexStage::Completed)
    );
}

#[tokio::test]
async fn management_service_keeps_kb_crud_documents_and_chunks_outside_routes() {
    let store = Arc::new(InMemoryKnowledgeBaseManagementStore::new());
    let service = KnowledgeBaseManagementService::new(store);
    let kb_id = KnowledgeBaseId::new("kb-1").expect("kb id");

    let created = service
        .create_kb(
            KnowledgeBaseCreateCommand::new(kb_id.clone(), "Docs", "embedding-1")
                .with_description(Some("Project docs".to_string()))
                .with_rerank_provider_id(Some("rerank-1".to_string()))
                .with_chunking(Some(256), Some(32)),
        )
        .await
        .expect("kb should create");

    assert_eq!(created.name, "Docs");
    assert_eq!(created.stats.doc_count, 0);
    assert_eq!(created.rerank_provider_id.as_deref(), Some("rerank-1"));

    let doc_id = DocumentId::new("doc-1").expect("doc id");
    service
        .upsert_document(KnowledgeDocument::new(
            doc_id.clone(),
            kb_id.clone(),
            "intro.txt",
            "txt",
        ))
        .await
        .expect("document should store");
    service
        .upsert_chunk(KnowledgeChunk::new(
            ChunkId::new("chunk-1").expect("chunk id"),
            kb_id.clone(),
            doc_id.clone(),
            0,
            "hello knowledge",
        ))
        .await
        .expect("chunk should store");

    let stats = service.stats_for(&kb_id).await.expect("stats should load");
    assert_eq!(stats.doc_count, 1);
    assert_eq!(stats.chunk_count, 1);

    let chunks = service
        .list_chunks_for_document(&doc_id)
        .await
        .expect("chunks should list");
    assert_eq!(
        chunks.chunks[0].char_count,
        "hello knowledge".chars().count()
    );

    let updated = service
        .update_kb(
            &kb_id,
            KnowledgeBaseUpdateCommand {
                name: Some("Reference".to_string()),
                top_m_final: Some(3),
                ..KnowledgeBaseUpdateCommand::default()
            },
        )
        .await
        .expect("kb should update")
        .expect("kb should exist");
    assert_eq!(updated.name, "Reference");
    assert_eq!(updated.top_m_final, 3);
    assert_eq!(updated.stats.chunk_count, 1);
}

#[tokio::test]
async fn sqlite_management_store_persists_kb_documents_and_chunks_after_reopen() {
    let db_path = std::env::temp_dir().join(format!(
        "astrbot-kb-management-test-{}.db",
        std::process::id()
    ));
    cleanup_sqlite_files(&db_path);

    let kb_id = KnowledgeBaseId::new("kb-1").expect("kb id");
    let doc_id = DocumentId::new("doc-1").expect("doc id");

    {
        let store = Arc::new(SqliteKnowledgeBaseManagementStore::new(
            SqliteJsonStore::open(&db_path).expect("sqlite store should open"),
        ));
        let service = KnowledgeBaseManagementService::new(store);
        service
            .create_kb(KnowledgeBaseCreateCommand::new(
                kb_id.clone(),
                "Docs",
                "embedding-1",
            ))
            .await
            .expect("kb should persist");
        service
            .upsert_document(KnowledgeDocument::new(
                doc_id.clone(),
                kb_id.clone(),
                "intro.txt",
                "txt",
            ))
            .await
            .expect("document should persist");
        service
            .upsert_chunk(KnowledgeChunk::new(
                ChunkId::new("chunk-1").expect("chunk id"),
                kb_id.clone(),
                doc_id.clone(),
                0,
                "dashboard knowledge",
            ))
            .await
            .expect("chunk should persist");
    }

    let store = Arc::new(SqliteKnowledgeBaseManagementStore::new(
        SqliteJsonStore::open(&db_path).expect("sqlite store should reopen"),
    ));
    let service = KnowledgeBaseManagementService::new(store);

    let catalog = service.list_kbs().await.expect("kbs should load");
    let stats = service.stats_for(&kb_id).await.expect("stats should load");
    let chunks = service
        .list_chunks_for_document(&doc_id)
        .await
        .expect("chunks should load");
    assert_eq!(catalog.knowledge_bases[0].kb_id, "kb-1");
    assert_eq!(stats.doc_count, 1);
    assert_eq!(stats.chunk_count, 1);
    assert_eq!(chunks.chunks[0].content, "dashboard knowledge");

    cleanup_sqlite_files(&db_path);
}

#[tokio::test]
async fn sqlite_document_repository_persists_profiles_documents_and_media_after_reopen() {
    let db_path = temp_sqlite_path("repository");
    cleanup_sqlite_files(&db_path);

    let kb_id = KnowledgeBaseId::new("kb-1").expect("kb id");
    let doc_id = DocumentId::new("doc-1").expect("doc id");
    {
        let repository = SqliteKnowledgeDocumentRepository::new(
            SqliteJsonStore::open(&db_path).expect("sqlite store should open"),
        );
        repository
            .upsert_kb(KnowledgeBaseProfile::new(
                kb_id.clone(),
                "Docs",
                "embedding-1",
            ))
            .await
            .expect("profile should persist");
        repository
            .upsert_document(KnowledgeDocument::new(
                doc_id.clone(),
                kb_id.clone(),
                "intro.txt",
                "txt",
            ))
            .await
            .expect("document should persist");
        repository
            .upsert_media(KnowledgeMedia {
                media_id: MediaId::new("media-1").expect("media id"),
                doc_id: doc_id.clone(),
                kb_id: kb_id.clone(),
                media_type: "image".to_string(),
                file_name: "diagram.png".to_string(),
                file_path: Some("kb-1/doc-1/diagram.png".to_string()),
                file_size: 4,
                mime_type: "image/png".to_string(),
            })
            .await
            .expect("media should persist");
    }

    let repository = SqliteKnowledgeDocumentRepository::new(
        SqliteJsonStore::open(&db_path).expect("sqlite store should reopen"),
    );
    assert_eq!(
        repository
            .get_kb(&kb_id)
            .await
            .expect("profile should load")
            .expect("profile should exist")
            .name,
        "Docs"
    );
    assert_eq!(
        repository
            .list_documents(&kb_id)
            .await
            .expect("documents should load")[0]
            .name,
        "intro.txt"
    );
    assert_eq!(
        repository
            .list_media(&doc_id)
            .await
            .expect("media should load")[0]
            .file_name,
        "diagram.png"
    );

    cleanup_sqlite_files(&db_path);
}

#[tokio::test]
async fn sqlite_vector_store_persists_search_and_delete_after_reopen() {
    let db_path = temp_sqlite_path("vector");
    cleanup_sqlite_files(&db_path);
    let kb_id = KnowledgeBaseId::new("kb-1").expect("kb id");
    let doc_id = DocumentId::new("doc-1").expect("doc id");

    {
        let store = SqliteVectorStore::new(
            SqliteJsonStore::open(&db_path).expect("sqlite store should open"),
        );
        store
            .upsert_chunks(vec![
                chunk("chunk-1", 0, "dashboard search", vec![1.0, 0.0]),
                chunk("chunk-2", 1, "provider settings", vec![0.0, 1.0]),
            ])
            .await
            .expect("vectors should persist");
    }

    let store = SqliteVectorStore::new(
        SqliteJsonStore::open(&db_path).expect("sqlite store should reopen"),
    );
    let hits = store
        .search(crate::VectorSearchRequest::new(vec![kb_id.clone()], vec![1.0, 0.0]).with_top_k(1))
        .await
        .expect("vectors should search after reopen");
    assert_eq!(hits[0].chunk.chunk_id.as_str(), "chunk-1");

    store
        .delete_chunk(&ChunkId::new("chunk-1").expect("chunk id"))
        .await
        .expect("chunk should delete");
    assert_eq!(store.count_chunks(&kb_id).await.expect("count"), 1);
    store
        .delete_document(&doc_id)
        .await
        .expect("document vectors should delete");
    assert_eq!(store.count_chunks(&kb_id).await.expect("count"), 0);

    cleanup_sqlite_files(&db_path);
}

#[tokio::test]
async fn ingestion_persists_metadata_vectors_and_hybrid_retrieval_after_reopen() {
    let db_path = temp_sqlite_path("ingestion");
    cleanup_sqlite_files(&db_path);
    let kb_id = KnowledgeBaseId::new("kb-1").expect("kb id");
    let doc_id = DocumentId::new("doc-1").expect("doc id");

    {
        let repository = Arc::new(SqliteKnowledgeDocumentRepository::new(
            SqliteJsonStore::open(&db_path).expect("sqlite store should open"),
        ));
        repository
            .upsert_kb(KnowledgeBaseProfile::new(
                kb_id.clone(),
                "Docs",
                "embedding-1",
            ))
            .await
            .expect("profile should persist");
        let vector_store = Arc::new(SqliteVectorStore::new(
            SqliteJsonStore::open(&db_path).expect("sqlite vector store should open"),
        ));
        let service = KnowledgeIngestionService::new(
            Arc::new(PlainTextParser),
            Arc::new(RecursiveCharacterChunker::with_separators([" "])),
            Arc::new(MockEmbeddingProvider::new(vec![0.5, 0.5])),
            Arc::new(VectorStorePersistencePort::new(vector_store)),
            repository,
            Arc::new(InMemoryKnowledgeMediaStore::new()),
        );

        let mut request = KnowledgeIngestionRequest::new(
            kb_id.clone(),
            doc_id.clone(),
            "intro.txt",
            "txt",
            "dashboard search knowledge base",
        )
        .with_embedding_provider_id("embedding-1");
        request.chunking = ChunkingOptions::new(16, 0).expect("chunking options");
        let outcome = service.ingest(request).await.expect("ingestion should run");
        assert_eq!(outcome.document.name, "intro.txt");
        assert_eq!(outcome.chunks.len(), outcome.chunk_count);
    }

    let repository = SqliteKnowledgeDocumentRepository::new(
        SqliteJsonStore::open(&db_path).expect("sqlite store should reopen"),
    );
    let chunk_count = repository
        .list_documents(&kb_id)
        .await
        .expect("documents should persist")[0]
        .chunk_count;
    assert!(chunk_count >= 2);

    let vector_store = Arc::new(SqliteVectorStore::new(
        SqliteJsonStore::open(&db_path).expect("sqlite vector store should reopen"),
    ));
    assert_eq!(
        vector_store
            .count_chunks(&kb_id)
            .await
            .expect("vectors should persist"),
        chunk_count
    );
    let retriever = HybridKnowledgeRetriever::new(
        vector_store.clone(),
        Arc::new(VectorStoreSparseRetriever::new(vector_store)),
    );
    let results = retriever
        .retrieve(
            KnowledgeRetrievalRequest::new("dashboard", vec![kb_id.clone()])
                .with_query_embedding(vec![0.5, 0.5])
                .with_limits(5, 5, 5, 2),
            Some(Arc::new(MockRerankProvider::new(vec![0.9, 0.1]))),
        )
        .await
        .expect("retrieval should run");
    assert!(!results.is_empty());
    assert_eq!(results[0].doc_name.as_deref(), Some("intro.txt"));

    cleanup_sqlite_files(&db_path);
}

#[tokio::test]
async fn provider_preflight_checks_embedding_dimension_and_rerank_smoke_test() {
    let service = KnowledgeProviderPreflightService::new(
        Arc::new(MockEmbeddingProvider::new(vec![0.25, 0.75])),
        Some(Arc::new(MockRerankProvider::new(vec![0.9]))),
    );

    let report = service
        .preflight(
            KnowledgeProviderPreflightRequest::new("embedding-1")
                .with_expected_embedding_dimension(2)
                .with_rerank_provider_id(Some("rerank-1".to_string())),
        )
        .await
        .expect("preflight should run");

    assert!(report.is_usable());
    assert_eq!(report.embedding.actual_dimension, Some(2));
    assert_eq!(
        report
            .rerank
            .as_ref()
            .expect("rerank preflight")
            .result_count,
        1
    );
}

#[tokio::test]
async fn upload_task_service_tracks_progress_results_and_failures() {
    let service =
        KnowledgeUploadTaskService::new(Arc::new(InMemoryKnowledgeUploadTaskStore::new()));
    let task_id = KnowledgeUploadTaskId::new("task-1").expect("task id");

    let started = service
        .start_task(task_id.clone(), KnowledgeUploadTaskKind::Upload, "kb-1", 2)
        .await
        .expect("task should start");
    assert_eq!(started.status, KnowledgeUploadTaskStatus::Pending);

    let progress = KnowledgeUploadProgress::queued(2).processing(
        1,
        "intro.txt",
        KnowledgeUploadStage::Embedding,
        3,
        5,
    );
    let updated = service
        .update_progress(&task_id, progress)
        .await
        .expect("progress should update")
        .expect("task should exist");
    assert_eq!(updated.status, KnowledgeUploadTaskStatus::Processing);
    assert_eq!(
        updated.progress.expect("progress").stage,
        KnowledgeUploadStage::Embedding
    );

    let completed = service
        .complete_task(
            &task_id,
            KnowledgeUploadTaskResult::new(vec!["doc-1".to_string()], 5),
        )
        .await
        .expect("task should complete")
        .expect("task should exist");
    assert_eq!(completed.status, KnowledgeUploadTaskStatus::Completed);
    assert_eq!(completed.result.expect("result").chunk_count, 5);

    let failed_id = KnowledgeUploadTaskId::new("task-2").expect("task id");
    service
        .start_task(failed_id.clone(), KnowledgeUploadTaskKind::Url, "kb-1", 1)
        .await
        .expect("task should start");
    let failed = service
        .fail_task(&failed_id, "download failed")
        .await
        .expect("task should fail")
        .expect("task should exist");
    assert_eq!(failed.status, KnowledgeUploadTaskStatus::Failed);
    assert_eq!(failed.error.as_deref(), Some("download failed"));
}

fn cleanup_sqlite_files(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
}

fn temp_sqlite_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("astrbot-kb-{label}-test-{}.db", std::process::id()))
}
