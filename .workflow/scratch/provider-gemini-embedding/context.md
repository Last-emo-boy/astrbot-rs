# M7-T2g Provider Parity: Gemini Embedding Provider

## Reference

- AstrBot registers `gemini_embedding` with `ProviderType.EMBEDDING`.
- AstrBot `GeminiEmbeddingProvider` calls Google GenAI embedding APIs and exposes `get_embedding`, `get_embeddings`, and `get_dim`.
- Google REST embeddings API exposes:
  - single text: `models/{model}:embedContent`
  - batch text: `models/{model}:batchEmbedContents`

## Rust Decision

- Add `GeminiEmbeddingProvider` behind the existing `EmbeddingProvider` trait.
- Register AstrBot-compatible provider type `gemini_embedding` through `ProviderRegistry::with_builtin_providers`.
- Use typed `GeminiEmbeddingConfig` with API key, base URL, model, timeout, custom headers, and dimensions.
- Registry fallback model is `gemini-embedding-001` to match the current public REST API, while explicit config can still pass any model string.
- Keep runtime, pipeline, and dashboard wiring deferred until non-chat provider runtime config is designed.

## Verification

- Targeted: `cargo test -p astrbot-provider --test gemini_embedding --test provider_registry`
- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
- `.workflow` JSON parse check
