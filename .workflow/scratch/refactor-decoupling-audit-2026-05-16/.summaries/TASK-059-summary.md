# TASK-059 Summary

## Result

Added a runtime config mutation service boundary in `crates/astrbot-runtime/src/config_service.rs`. It centralizes runtime config read, schema access, JSON validation, write execution, and reload/restart planning. Web management handlers no longer need direct file write logic for mutable config APIs.

Added a UMOP routing boundary in `crates/astrbot-runtime/src/config_route.rs`. It models `platform_id:message_type:session_id` route patterns, preserves colons inside session IDs, supports empty-component and `*` wildcard matching, and can replace, update, resolve, and delete routes without dashboard state.

Added `crates/astrbot-web/src/management/config.rs` as a thin dashboard config route layer. It exposes schema, preview, and apply handlers that delegate to `RuntimeConfigService`; `ManagementApiState` optionally carries the service so existing management status routes remain compatible.

## Files

- `crates/astrbot-runtime/src/config_route.rs`
- `crates/astrbot-runtime/src/config_service.rs`
- `crates/astrbot-runtime/src/config/schema.rs`
- `crates/astrbot-runtime/src/lib.rs`
- `crates/astrbot-runtime/src/tests.rs`
- `crates/astrbot-runtime/src/tests/config_route.rs`
- `crates/astrbot-runtime/src/tests/config_service.rs`
- `crates/astrbot-web/Cargo.toml`
- `crates/astrbot-web/src/lib.rs`
- `crates/astrbot-web/src/management/mod.rs`
- `crates/astrbot-web/src/management/config.rs`
- `crates/astrbot-web/src/tests/management.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-059.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.summaries/TASK-059-summary.md`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`
- `.workflow/.csv-wave/20260517-execute-scratch-task-059/wave-1.csv`

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-runtime`
- `cargo test -p astrbot-web`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all --check`
