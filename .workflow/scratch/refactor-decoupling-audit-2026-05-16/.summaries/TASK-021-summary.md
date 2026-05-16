# TASK-021 Summary - WebChat Test Boundary

Status: completed

## Scope

Split the WebChat HTTP test module by behavior area so future auth, streaming, storage, and attachment tests have clear destinations instead of growing one broad `tests.rs`.

## Changes

- Replaced `crates/astrbot-web/src/tests.rs` with a test facade.
- Added `tests/support.rs` for shared WebChat fixtures, router request helpers, JSON response parsing, and runtime sent-message waiting.
- Added `tests/submit.rs` for text/image/message-part submit route coverage.
- Added `tests/message_parts.rs` for reply-only, empty payload, and non-image media validation.
- Added `tests/history.rs` for recorded message history readback.
- Added `tests/server.rs` for bound TCP server and runtime-history smoke coverage.

## AstrBot Reference

This keeps Rust WebChat tests aligned with AstrBot's dedicated WebChat message-part helper boundary in `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/message_parts_helper.py`, while separating HTTP route behavior from live server behavior.

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-web`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
