# TASK-063 Summary

## Result

Added scoped file-token storage, dashboard static asset policy, and management download route boundaries. HTTP download routes now consume opaque tokens through a typed service and never accept raw filesystem paths from request parameters.

`astrbot-storage` now has `file_token.rs` with scoped token records, expiry checks, single-use/reusable behavior, and an in-memory repository port. `astrbot-runtime` now has `dashboard_assets.rs` for WebUI asset root selection, bundled/user/explicit precedence, SPA index route mapping, and traversal-safe asset path resolution. `astrbot-web` now has `management/files.rs` with `ManagementFileDownloadState`, `ScopedDownloadService`, and `/api/management/files/{token}` response handling.

## Files

- `crates/astrbot-storage/src/lib.rs`
- `crates/astrbot-storage/src/file_token.rs`
- `crates/astrbot-runtime/src/lib.rs`
- `crates/astrbot-runtime/src/dashboard_assets.rs`
- `crates/astrbot-web/src/lib.rs`
- `crates/astrbot-web/src/management/mod.rs`
- `crates/astrbot-web/src/management/files.rs`
- `crates/astrbot-web/src/tests/management.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-063.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.summaries/TASK-063-summary.md`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`
- `.workflow/.csv-wave/20260517-execute-scratch-task-063/wave-1.csv`

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-web`
- `cargo test -p astrbot-storage`
- `cargo test -p astrbot-runtime`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all --check`
