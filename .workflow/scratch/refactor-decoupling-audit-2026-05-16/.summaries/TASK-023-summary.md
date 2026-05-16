# TASK-023 Summary - Platform Transport Boundary

Status: completed

## Scope

Define platform transport, session metadata, and media upload/download boundary types before adding real OneBot, QQ, or WeChat network parity.

## Changes

- Added `adapters/common/transport.rs` with `PlatformTransport`, `PlatformTransportKind`, `PlatformTransportState`, and `NoopTransport`.
- Added `adapters/common/media.rs` with `PlatformMediaUpload`, `PlatformMediaReference`, source/kind enums, and `PlatformMediaUploadClient`.
- Added `adapters/onebot/session.rs` with `OneBotSession` and `OneBotSessionKind` to keep OneBot session/conversation metadata outside message conversion.
- Added `adapters/onebot/transport.rs` with `OneBotTransport` and `OneBotTransportMode` for in-process and reverse-WebSocket lifecycle state.
- Updated `OneBotPlatform` so `run()` and `terminate()` delegate through its transport boundary while existing submit behavior remains unchanged.
- Re-exported the new boundary types through `astrbot-platform`.
- Added focused tests for transport state, media upload DTOs, OneBot session metadata, and manager run/terminate over OneBot transport.

## AstrBot Reference

This follows AstrBot's separation pressure in `PlatformManager`, `aiocqhttp_platform_adapter.py`, and `qqofficial_platform_adapter.py`: long-running adapter tasks, session scene/message IDs, outbound sending, and media upload need boundaries before real platform parity grows.

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-platform`
- `cargo clippy -p astrbot-platform -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
