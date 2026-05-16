# TASK-008 Summary

Status: completed

Split `astrbot-provider` second-pass provider boundaries by capability while preserving public re-exports and registry/manager behavior.

Changes:

- `config.rs` is now a facade re-exporting capability config modules under `config/`.
- `factories.rs` is now a facade re-exporting chat, speech, TTS, embedding, and rerank factory modules, with shared provider option parsing in `factories/common.rs`.
- `manager.rs` keeps `ProviderManager` as the public facade while moving `ProviderManagerConfigSet`, termination, and each capability trait routing implementation into `manager/` submodules.

AstrBot alignment:

- Mirrors AstrBot provider manager capability buckets (`provider_insts`, `stt_provider_insts`, `tts_provider_insts`, `embedding_provider_insts`, `rerank_provider_insts`) with typed Rust modules.
- Keeps provider-specific payload/config mapping in concrete factory/provider modules instead of centralizing it in the registry.

Verification:

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
