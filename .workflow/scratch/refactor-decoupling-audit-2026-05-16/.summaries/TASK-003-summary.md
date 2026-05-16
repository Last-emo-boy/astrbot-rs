# TASK-003 Summary - Provider Public Trait/Test Split

Status: completed

## Result

`crates/astrbot-provider/src/lib.rs` is now a public facade for provider traits and DTOs. Capability definitions moved into focused modules:

- `chat.rs`
- `speech.rs`
- `tts.rs`
- `embedding.rs`
- `rerank.rs`
- `mock.rs`

`crates/astrbot-provider/tests/provider_registry.rs` now holds shared test helpers only, with capability tests split under `tests/provider_registry/`:

- `chat.rs`
- `speech.rs`
- `tts.rs`
- `embedding.rs`
- `rerank.rs`
- `lifecycle.rs`

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

All passed.

## Follow-Up Audit

The next decoupling wave should stay focused on `TASK-004` through `TASK-006`: web HTTP boundary, core message domain, and pipeline context. Additional second-pass items were added to `plan.json` for runtime provider config/tests, provider config/factories, plugin SDK/sandbox, platform adapter transport/message conversion, large integration tests, and WebChat attachment/message-part services.
