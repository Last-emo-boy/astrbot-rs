# M7-T2h Provider Parity: Rerank Boundary

## Reference

- AstrBot defines `RerankProvider.rerank(query, documents, top_n)` returning `list[RerankResult]`.
- `RerankResult` carries `index` and `relevance_score`.
- `ProviderManager` stores rerank providers in a separate `rerank_provider_insts` capability bucket.
- Concrete AstrBot adapters include `vllm_rerank`, `xinference_rerank`, and `bailian_rerank`.

## Rust Decision

- Add a provider-crate-only rerank boundary first: `RerankRequest`, `RerankDocumentScore`, `RerankResponse`, `RerankProvider`, and `MockRerankProvider`.
- Add `RerankProviderConfig`, `register_rerank_provider`, `build_rerank_provider`, and `ProviderManager::from_rerank_configs`.
- Let `ProviderManager` implement `RerankProvider` routing by requested provider ID or default rerank provider ID.
- Keep concrete VLLM/Xinference/Bailian HTTP adapters, runtime config, pipeline, and dashboard wiring deferred.

## Verification

- Targeted: `cargo test -p astrbot-provider --test provider_registry`
- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
- `.workflow` JSON parse check
