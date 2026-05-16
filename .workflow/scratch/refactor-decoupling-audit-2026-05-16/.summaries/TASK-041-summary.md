# TASK-041 Summary

## Result

Split the core event bus into a stable facade plus focused routing and logging boundaries. `EventBus::new(receiver, executor)` remains compatible with the existing single-scheduler path, while `EventBus::with_router` and `EventBus::with_logger` expose testable seams for future config/session-aware scheduler selection and event outline logging.

## Files

- removed `crates/astrbot-core/src/event.rs`
- `crates/astrbot-core/src/event/mod.rs`
- `crates/astrbot-core/src/event/router.rs`
- `crates/astrbot-core/src/event/logging.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-041.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-core`
- `cargo test -p astrbot-pipeline`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
