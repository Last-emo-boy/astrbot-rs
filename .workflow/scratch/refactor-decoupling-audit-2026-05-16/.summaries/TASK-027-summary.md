# TASK-027 Summary - Provider Registry Builtin Boundary

## Outcome

Split `ProviderRegistry` internals while keeping `astrbot_provider::ProviderRegistry` as the stable public entry point.

New modules:

- `registry/builtins.rs`: deterministic built-in provider registration for chat, STT, TTS, embedding, and rerank providers.
- `registry/factory.rs`: per-capability provider factory trait object aliases.
- `registry/metadata.rs`: provider adapter metadata index, duplicate checks, and capability lookup helpers.
- `registry/errors.rs`: shared duplicate-registration and capability-mismatch error construction.

## Integration

- `ProviderRegistry::with_builtin_providers()` and `with_builtin_chat_providers()` now delegate to `registry::builtins`.
- Registration methods still attach metadata before storing per-capability factories.
- Build methods still return the same capability mismatch and unregistered-provider messages through shared helpers.
- No concrete provider payload mapping or runtime config mapping moved into the registry.

## AstrBot Reference

Compared against:

- `E:/Playground/Astrbot/astrbot/core/provider/register.py`
- `E:/Playground/Astrbot/astrbot/core/provider/manager.py`

Rust keeps AstrBot's provider type map and capability-bucket idea, but makes factory dispatch and metadata indexing typed and module-local.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
