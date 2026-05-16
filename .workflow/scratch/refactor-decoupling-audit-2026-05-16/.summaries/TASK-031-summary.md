# TASK-031 Summary - Knowledge Base Boundary

## Outcome

Added a new `astrbot-kb` crate and registered it in the workspace. The crate defines typed KB/RAG boundaries before knowledge-base parity reaches pipeline stages, provider adapters, or agent request decorators.

New modules:

- `types.rs`: validated KB, document, media, and chunk identifiers plus `KnowledgeChunk`.
- `document.rs`: knowledge-base profile, document, media, and stats DTOs.
- `parser.rs`: parser port, parse result, media item DTO, and plain-text parser.
- `chunking.rs`: document chunker port and recursive character chunker with overlap.
- `embedding.rs`: embedding orchestration that consumes `astrbot-provider` embedding providers without making vector storage own providers.
- `vector_store.rs`: vector-store port, search request/result, and in-memory test implementation.
- `rank_fusion.rs`: reciprocal rank fusion boundary.
- `retrieval.rs`: sparse retrieval port, in-memory sparse retriever, hybrid dense/sparse retrieval, and optional rerank integration.
- `formatter.rs`: retrieval-context formatter for agent request decoration.

## Integration

- `Cargo.toml` now includes `crates/astrbot-kb`.
- Embedding and rerank capabilities remain in `astrbot-provider`; `astrbot-kb` only depends on those traits as ports.
- No runtime, pipeline, dashboard, or real DB/vector backend wiring was added in this pass.

## AstrBot Reference

Compared against:

- `E:/Playground/Astrbot/astrbot/core/knowledge_base/kb_mgr.py`
- `E:/Playground/Astrbot/astrbot/core/knowledge_base/kb_helper.py`
- `E:/Playground/Astrbot/astrbot/core/knowledge_base/models.py`
- `E:/Playground/Astrbot/astrbot/core/knowledge_base/chunking/base.py`
- `E:/Playground/Astrbot/astrbot/core/knowledge_base/chunking/recursive.py`
- `E:/Playground/Astrbot/astrbot/core/knowledge_base/parsers/base.py`
- `E:/Playground/Astrbot/astrbot/core/knowledge_base/retrieval/manager.py`
- `E:/Playground/Astrbot/astrbot/core/knowledge_base/retrieval/rank_fusion.py`
- `E:/Playground/Astrbot/astrbot/core/knowledge_base/retrieval/sparse_retriever.py`

Rust keeps AstrBot's separation between parsing, chunking, embedding, vector search, sparse retrieval, rank fusion, rerank, and final context formatting, but expresses the first pass as typed traits and DTOs.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-kb`
- `cargo test -p astrbot-provider`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
