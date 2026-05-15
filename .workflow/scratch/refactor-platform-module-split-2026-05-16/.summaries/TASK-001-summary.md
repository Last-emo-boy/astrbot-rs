# TASK-001 Summary

Split `astrbot-platform` into module-level boundaries:

- `core.rs` for traits/config/shared sink types.
- `built.rs` for `BuiltPlatform`.
- `registry.rs` for `PlatformRegistry` and built-in platform factories.
- `manager.rs` for `PlatformManager`.
- `adapters/` for concrete platform implementations.
- `lib.rs` for re-exports only.

This prepares the platform layer for future WeChat, QQ, and OneBot transport work without letting concrete adapter code accumulate in the crate root.
