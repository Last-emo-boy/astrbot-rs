# M7-T2j Provider Parity: Bailian Rerank Provider

## Reference

- AstrBot registers `bailian_rerank` with `ProviderType.RERANK`.
- AstrBot `BailianRerankProvider` posts to DashScope text-rerank endpoint from `rerank_api_base`.
- Payload shape is `model`, `input.query`, `input.documents`, and optional `parameters.top_n`, `parameters.return_documents`, `parameters.instruct`.
- `instruct` is only sent for `qwen3-rerank`.
- Response results map from `output.results[]` into `RerankResult(index, relevance_score)`, defaulting missing scores to `0.0`.

## Rust Decision

- Add `BailianRerankProvider` behind the existing `RerankProvider` trait.
- Register AstrBot-compatible provider type `bailian_rerank` through `ProviderRegistry::with_builtin_providers`.
- Keep provider-specific `return_documents` and `instruct` on `BailianRerankConfig` instead of widening the generic `RerankProviderConfig`.
- Keep runtime/pipeline/dashboard wiring deferred until non-chat provider runtime config is designed.
- Require explicit API key in Rust provider construction rather than reading `DASHSCOPE_API_KEY` inside provider code; env resolution should remain a runtime/config concern.

## Verification

- PASS: `cargo test -p astrbot-provider`
- PASS: `cargo fmt --all --check`
- PASS: `cargo test --workspace`
- PASS: `cargo clippy --workspace -- -D warnings`
- PASS: `cargo run -p astrbot-cli`
- PASS: `.workflow` JSON parse check
