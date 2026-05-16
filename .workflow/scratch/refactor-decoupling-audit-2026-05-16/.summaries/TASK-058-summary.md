# TASK-058 Summary

## Result

Defined management dashboard authentication and OpenAPI API-key boundaries without embedding auth logic into individual management handlers. The default `management_router` remains unauthenticated for compatibility, while `management_router_with_auth` attaches reusable middleware for bearer-token protected management routes.

Added a storage-side API-key repository port and an in-memory implementation so API keys can be issued, listed, looked up by hash, and revoked outside web route state. Added typed OpenAPI scope modeling, issued/presented key helpers, hashing, extraction, and authorization decisions in the web management boundary.

## Files

- `Cargo.lock`
- `crates/astrbot-storage/src/api_key.rs`
- `crates/astrbot-storage/src/lib.rs`
- `crates/astrbot-storage/src/schema.rs`
- `crates/astrbot-web/Cargo.toml`
- `crates/astrbot-web/src/lib.rs`
- `crates/astrbot-web/src/management/mod.rs`
- `crates/astrbot-web/src/management/auth.rs`
- `crates/astrbot-web/src/management/api_key.rs`
- `crates/astrbot-web/src/server.rs`
- `crates/astrbot-web/src/tests/management.rs`
- `crates/astrbot-web/src/tests/support.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-058.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.summaries/TASK-058-summary.md`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`
- `.workflow/.csv-wave/20260517-execute-scratch-task-058/wave-1.csv`

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-storage`
- `cargo test -p astrbot-web`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all --check`
