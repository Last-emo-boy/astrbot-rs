# TASK-007 Summary

`TASK-007` is completed. Runtime provider config mapping is now split by provider capability:

- `provider_config.rs`: facade re-exports.
- `provider_config/chat.rs`: chat provider runtime DTOs and mapping.
- `provider_config/speech.rs`: STT provider runtime DTOs and mapping.
- `provider_config/tts.rs`: TTS provider runtime DTOs and mapping.
- `provider_config/embedding.rs`: embedding provider runtime DTOs and mapping.
- `provider_config/rerank.rs`: rerank provider runtime DTOs and mapping.

Runtime tests are now grouped by behavior area:

- `tests.rs`: test module facade and shared helpers.
- `tests/message_loop.rs`: mock message loop, command plugin, and provider preference flow.
- `tests/provider_config.rs`: provider config defaults and capability bucket mapping.
- `tests/policy.rs`: session status, provider fallback, result decoration, and content safety.
- `tests/platform.rs`: mock, console, WebChat, and OneBot platform wiring.
- `tests/lifecycle.rs`: handle start/stop/restart and restart state policy.
- `tests/config_io.rs`: config file creation and default writeback.

This preserves current `astrbot_runtime::*` provider config re-exports while mirroring AstrBot's separate provider capability buckets in Rust modules.

Verification passed: `cargo fmt --all --check`, `cargo test -p astrbot-runtime`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings`.
