# M7-T2e Provider Parity: Embedding Boundary

## Reference

- AstrBot exposes `EmbeddingProvider` with `get_embedding`, `get_embeddings`, and `get_dim`.
- AstrBot registers `openai_embedding` and `gemini_embedding` as `ProviderType.EMBEDDING`.
- AstrBot `ProviderManager` keeps `embedding_provider_insts` separate from chat/STT/TTS providers.

## Rust Decision

- Add a provider-crate-only embedding boundary first: `EmbeddingRequest`, `EmbeddingResponse`, `EmbeddingProvider`, and `MockEmbeddingProvider`.
- Add `EmbeddingProviderConfig`, `register_embedding_provider`, `build_embedding_provider`, and `ProviderManager::from_embedding_configs`.
- Let `ProviderManager` implement `EmbeddingProvider` routing by requested provider ID or default embedding provider ID, mirroring the existing chat manager pattern.
- Keep runtime, pipeline, dashboard, and concrete OpenAI/Gemini embedding HTTP providers deferred until the trait/config/manager boundary is stable.

## Verification

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
