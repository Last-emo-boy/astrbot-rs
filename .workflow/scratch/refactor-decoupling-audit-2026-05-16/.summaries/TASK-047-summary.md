# TASK-047 Summary

## Result

Introduced the `astrbot-computer` crate with separate sandbox lifecycle, component capability, and skill synchronization boundaries. `booter.rs` models local/remote booter configuration, session lifecycle, and registry ports. `components.rs` maps shell, Python, filesystem, browser, and skill-lifecycle capabilities into typed `astrbot-tool` declarations. `skill_sync.rs` separates upload/apply/scan/cache-refresh planning so sandbox skill synchronization is testable without booting a real sandbox.

## Files

- `Cargo.toml`
- `Cargo.lock`
- `crates/astrbot-computer/Cargo.toml`
- `crates/astrbot-computer/src/lib.rs`
- `crates/astrbot-computer/src/booter.rs`
- `crates/astrbot-computer/src/components.rs`
- `crates/astrbot-computer/src/skill_sync.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-047.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-computer`
- `cargo test -p astrbot-plugin`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
