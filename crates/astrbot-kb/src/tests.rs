use std::sync::Arc;

use astrbot_provider::{MockEmbeddingProvider, MockRerankProvider};

use crate::{
    ChunkId, ChunkingOptions, DocumentChunker, DocumentId, EmbeddedKnowledgeChunk,
    HybridKnowledgeRetriever, InMemorySparseRetriever, InMemoryVectorStore, KnowledgeBaseId,
    KnowledgeChunk, KnowledgeContextFormatter, KnowledgeRetrievalRequest, KnowledgeRetriever,
    RecursiveCharacterChunker, RetrievalContextFormatter, VectorStore, embed_chunks,
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
