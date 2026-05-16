# TASK-044 Summary

## Result

Introduced `astrbot-persona` and `astrbot-skill` workspace crates. `PersonaManager` now owns default/session persona resolution, disabled persona handling, WebChat special defaults, and folder metadata behind a repository port. `SkillCatalog` separates skill descriptor activation from plugin handler registration, and `SkillPromptRenderer` keeps skill prompt composition policy testable before agent request decoration consumes it.

## Files

- `Cargo.toml`
- `Cargo.lock`
- `crates/astrbot-persona/Cargo.toml`
- `crates/astrbot-persona/src/lib.rs`
- `crates/astrbot-persona/src/manager.rs`
- `crates/astrbot-skill/Cargo.toml`
- `crates/astrbot-skill/src/lib.rs`
- `crates/astrbot-skill/src/catalog.rs`
- `crates/astrbot-skill/src/prompt.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-044.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-persona`
- `cargo test -p astrbot-skill`
- `cargo test -p astrbot-plugin`
- `cargo test -p astrbot-pipeline`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
