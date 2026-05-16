# Platform Registry Context

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/platform/register.py`
  - `platform_cls_map` maps platform adapter type to adapter class.
  - `register_platform_adapter()` records metadata and prevents duplicate adapter types.
- `E:/Playground/Astrbot/astrbot/core/platform/manager.py`
  - `PlatformManager` reads configured platform entries, skips disabled entries, validates platform IDs, instantiates adapters and stores instances by ID/client ID.
  - Platform loading is owned by lifecycle/manager code, not by CLI.

## Rust Increment

- Added `PlatformConfig`, `PlatformBuildContext`, `BuiltPlatform`, `PlatformRegistry` and `PlatformManager` to `astrbot-platform`.
- Added built-in `mock` platform registration through `PlatformRegistry::with_builtin_platforms()`.
- Added duplicate platform type protection and configured platform ID validation.
- Added `RuntimePlatformConfig` to `astrbot-runtime`.
- `AstrbotRuntime::initialize()` now constructs `PlatformManager` from config instead of directly constructing `MockPlatform`.
- Runtime smoke helpers now route through platform manager:
  - `emit_mock_text()`
  - `emit_mock_text_on(platform_id, ...)`
  - `sent_messages()`
  - `sent_messages_for(platform_id)`

## Result

- Platform manager tests cover built-in type registration, duplicate type rejection, disabled config skipping and mock event emission.
- Runtime tests cover platform construction from config and preserve existing provider/plugin/message loop behavior.
- Verification passed:
  - `cargo fmt --all --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo run -p astrbot-cli`

## Next

- Add real platform adapter crates behind `PlatformRegistry`.
- Add runtime start/stop task management around `PlatformManager::run_all()` and `EventBus::run()`.
- Add platform reload support once config persistence is more complete.
