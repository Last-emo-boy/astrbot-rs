# M7-T2r Runtime Non-Chat Provider Config

## Reference

- AstrBot `ProviderManager` keeps chat, STT, TTS, embedding, and rerank providers in separate capability buckets.
- AstrBot still exposes one manager lifecycle for loading and termination; non-chat providers are not forced through chat request flow.

## Rust Decision

- Add `ProviderManagerConfigSet` so `ProviderManager` can build all capability buckets in one pass.
- Keep existing `from_chat_configs`, `from_speech_to_text_configs`, `from_text_to_speech_configs`, `from_embedding_configs`, and `from_rerank_configs` as focused convenience constructors.
- Extend `RuntimeConfig` with separate non-chat provider arrays and optional default IDs.
- Map runtime non-chat configs into typed provider configs for STT, TTS, Embedding, and Rerank.
- Runtime still only injects a chat provider into `PipelineContext`; non-chat providers are available through `runtime.provider_manager()` and remain out of pipeline/dashboard wiring until a concrete workflow needs them.

## Verification

- PASS: `cargo fmt --all`
- PASS: `cargo test -p astrbot-provider -p astrbot-runtime`
- PASS: `cargo fmt --all --check`
- PASS: `.workflow` JSON parse check
- PASS: `cargo test --workspace`
- PASS: `cargo clippy --workspace -- -D warnings`
- PASS: `cargo run -p astrbot-cli`
