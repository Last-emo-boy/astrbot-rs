# TASK-088 Summary

Completed at: 2026-05-17T16:50:19+08:00

## Scope

Split platform core contracts, build configuration, recording sink DTOs, and platform ID validation out of a single mixed `core.rs` facade.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/platform/platform.py`
- `E:/Playground/Astrbot/astrbot/core/platform/platform_metadata.py`
- `E:/Playground/Astrbot/astrbot/core/platform/register.py`
- `E:/Playground/Astrbot/astrbot/core/platform/manager.py`

## Changes

- Replaced `crates/astrbot-platform/src/core.rs` with `core/mod.rs`.
- Added `core/adapter.rs` for `PlatformAdapter` and `MessageRecorder`.
- Added `core/config.rs` for platform type constants and `PlatformConfig`.
- Added `core/build_context.rs` for event-sender build context.
- Added `core/recording.rs` for `RecordingSink`, `SentMessage`, `StreamedMessage`, and history/sink impls.
- Added `core/validation.rs` plus focused tests for platform ID validation.

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-platform`
- `cargo clippy -p astrbot-platform -- -D warnings`
- `cargo test --workspace`

## Next

Next pending task is `TASK-089`.
