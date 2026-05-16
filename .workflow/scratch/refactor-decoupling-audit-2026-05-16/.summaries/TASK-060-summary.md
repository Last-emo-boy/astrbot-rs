# TASK-060 Summary

## Result

Added a plugin marketplace boundary under `crates/astrbot-plugin/src/market`. It models registry sources, custom source MD5 URLs, package cache records, package descriptors, install sources, compatibility data, and README/changelog document DTOs separately from the plugin loader lifecycle.

Added `market/update.rs` with side-effect-free install, update, and uninstall operation plans. Plans describe download, unpack, loader reload, compatibility, and optional config/data deletion requirements without performing dynamic loading or filesystem changes.

Added `crates/astrbot-web/src/management/plugin_market.rs` as a thin dashboard route boundary. Management state can carry a typed in-memory market catalog, and routes return catalog data or install/update/uninstall plans without invoking plugin loader, download, unzip, or install logic.

## Files

- `Cargo.lock`
- `crates/astrbot-plugin/Cargo.toml`
- `crates/astrbot-plugin/src/lib.rs`
- `crates/astrbot-plugin/src/market/mod.rs`
- `crates/astrbot-plugin/src/market/update.rs`
- `crates/astrbot-plugin/src/tests/mod.rs`
- `crates/astrbot-plugin/src/tests/market.rs`
- `crates/astrbot-web/src/lib.rs`
- `crates/astrbot-web/src/management/mod.rs`
- `crates/astrbot-web/src/management/plugin_market.rs`
- `crates/astrbot-web/src/tests/management.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-060.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.summaries/TASK-060-summary.md`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`
- `.workflow/.csv-wave/20260517-execute-scratch-task-060/wave-1.csv`

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-plugin`
- `cargo test -p astrbot-web`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all --check`
