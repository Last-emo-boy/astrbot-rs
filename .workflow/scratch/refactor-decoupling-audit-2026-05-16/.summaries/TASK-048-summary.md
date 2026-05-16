# TASK-048 Summary

## Result

Split plugin dependency handling into `astrbot-plugin::dependency` with separate installer, import environment, and conflict/redaction boundaries. Dependency plans now run through a replaceable `PluginDependencyPlanInstaller` port from `PluginLoader`, while the legacy `PluginDependencyInstaller::ensure_dependencies` surface remains available for compatibility. Import environment policy models isolated dependency roots, site-packages preference, and packaged Python runtime patch behavior without embedding package installation in loader state transitions.

## Files

- `crates/astrbot-plugin/src/dependency/mod.rs`
- `crates/astrbot-plugin/src/dependency/installer.rs`
- `crates/astrbot-plugin/src/dependency/environment.rs`
- `crates/astrbot-plugin/src/dependency/conflict.rs`
- `crates/astrbot-plugin/src/loader/dependency.rs`
- `crates/astrbot-plugin/src/loader/mod.rs`
- `crates/astrbot-plugin/src/lib.rs`
- `crates/astrbot-plugin/src/tests.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-048.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-plugin`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
