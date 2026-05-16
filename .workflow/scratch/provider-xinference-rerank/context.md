# M7-T2l Provider Parity: Xinference Rerank Provider

## Reference

- AstrBot registers `xinference_rerank` with `ProviderType.RERANK`.
- AstrBot initializes a Xinference client from `rerank_api_base`, `rerank_api_key`, `rerank_model`, `timeout`, and `launch_model_if_not_running`.
- Initialization lists running models, resolves the model UID by `model_name`, optionally launches the rerank model, then reranks through the model handle.
- Rerank results map `results[].index` and `results[].relevance_score` into `RerankResult`.

## Rust Decision

- Add `XinferenceRerankProvider` behind the existing `RerankProvider` trait.
- Register AstrBot-compatible provider type `xinference_rerank` through `ProviderRegistry::with_builtin_providers`.
- Keep model UID discovery lazy inside the provider so runtime/pipeline do not gain provider-specific lifecycle coupling.
- Support both AstrBot/Xinference UID map model-list responses and OpenAI-like REST `data` list responses.
- Keep runtime/pipeline/dashboard wiring deferred until non-chat provider runtime config is designed.

## Verification

- PASS: `cargo test -p astrbot-provider`
- PASS: `cargo fmt --all --check`
- PASS: `cargo test --workspace`
- PASS: `cargo clippy --workspace -- -D warnings`
- PASS: `cargo run -p astrbot-cli`
- PASS: `.workflow` JSON parse check
