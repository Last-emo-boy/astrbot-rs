# M7-T2f Provider Parity: OpenAI Embedding Provider

## Reference

- AstrBot registers `openai_embedding` with `ProviderType.EMBEDDING`.
- AstrBot `OpenAIEmbeddingProvider` calls OpenAI `/embeddings` with `input`, `model`, and `dimensions`.
- AstrBot defaults:
  - `embedding_api_base`: `https://api.openai.com/v1`, appending `/v1` when a custom base does not include it.
  - `embedding_model`: `text-embedding-3-small`.
  - `embedding_dimensions`: `1024`.

## Rust Decision

- Add `OpenAiEmbeddingProvider` behind the existing `EmbeddingProvider` trait instead of expanding `ChatRequest`.
- Register AstrBot-compatible provider type `openai_embedding` through `ProviderRegistry::with_builtin_providers`.
- Keep runtime/pipeline/dashboard wiring deferred; this is a provider-crate concrete adapter that can be built through `ProviderManager::from_embedding_configs`.

## Verification

- Targeted: `cargo test -p astrbot-provider --test openai_embedding --test provider_registry`
- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
- `.workflow` JSON parse check
