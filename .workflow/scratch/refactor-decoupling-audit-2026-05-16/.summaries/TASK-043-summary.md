# TASK-043 Summary

## Result

Introduced the `astrbot-conversation` crate with conversation directory, platform message history, and persona-conversation link domain services. Platform/WebChat history now goes through `PlatformMessageHistoryService`; storage repositories remain behind the conversation boundary while WebChat history response shape stays unchanged.

## Files

- `Cargo.toml`
- `Cargo.lock`
- `crates/astrbot-conversation/Cargo.toml`
- `crates/astrbot-conversation/src/lib.rs`
- `crates/astrbot-conversation/src/conversation.rs`
- `crates/astrbot-conversation/src/message_history.rs`
- `crates/astrbot-conversation/src/persona_link.rs`
- `crates/astrbot-platform/Cargo.toml`
- `crates/astrbot-platform/src/adapters/webchat/mod.rs`
- `crates/astrbot-platform/src/core.rs`
- `crates/astrbot-web/Cargo.toml`
- `crates/astrbot-web/src/history.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-043.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-conversation`
- `cargo test -p astrbot-platform`
- `cargo test -p astrbot-web`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
