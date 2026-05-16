# TASK-042 Summary

## Result

Introduced typed provider selection state for default and session-scoped provider choices, added provider-selection change hook events to `ProviderManager`, and moved runtime provider selection/config-set wiring into `crates/astrbot-runtime/src/provider_selection.rs`. Existing provider request routing still resolves explicit request provider IDs first and falls back to the selected default provider for each capability.

## Files

- `crates/astrbot-provider/src/lib.rs`
- `crates/astrbot-provider/src/manager.rs`
- `crates/astrbot-provider/src/manager/hooks.rs`
- `crates/astrbot-provider/src/selection.rs`
- `crates/astrbot-runtime/src/assembly.rs`
- `crates/astrbot-runtime/src/lib.rs`
- `crates/astrbot-runtime/src/provider_selection.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-042.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo test -p astrbot-runtime`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
