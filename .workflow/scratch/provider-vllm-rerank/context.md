# M7-T2i Provider Parity: VLLM Rerank Provider

## Reference

- AstrBot registers `vllm_rerank` with `ProviderType.RERANK`.
- AstrBot `VLLMRerankProvider` posts to `{rerank_api_base}/v1/rerank`.
- Payload fields are `query`, `documents`, `model`, and optional `top_n`.
- Response results map to `RerankResult(index, relevance_score)`.

## Rust Decision

- Add `VllmRerankProvider` behind the existing `RerankProvider` trait.
- Register AstrBot-compatible provider type `vllm_rerank` through `ProviderRegistry::with_builtin_providers`.
- Keep request/response mapping local to provider crate and leave runtime/pipeline/dashboard wiring deferred.
- Keep Xinference and Bailian rerank adapters deferred because they need additional model lifecycle or vendor-specific response handling.

## Verification

- Targeted: `cargo test -p astrbot-provider --test vllm_rerank --test provider_registry`
- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
- `.workflow` JSON parse check
