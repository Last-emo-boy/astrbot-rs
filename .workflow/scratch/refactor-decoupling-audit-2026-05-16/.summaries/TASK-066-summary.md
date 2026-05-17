# TASK-066 Summary

## Result

Added MCP transport runtime boundaries under `crates/astrbot-mcp/src/transport`. The module defines stdio/SSE/streamable HTTP endpoint plans, stdio process supervision plans, transport runtime ports, tolerant JSON-RPC stdout parsing that filters noisy lines, and reconnect backoff decisions.

Added MCP bridge registration boundaries under `crates/astrbot-mcp/src/bridge`. The bridge converts MCP tools plus synthetic resource and prompt bridge operations into `astrbot-tool` catalog descriptors with `ToolSource::Mcp`, keeping MCP registration outside plugin/provider ownership.

## Files

- `Cargo.lock`
- `crates/astrbot-mcp/Cargo.toml`
- `crates/astrbot-mcp/src/lib.rs`
- `crates/astrbot-mcp/src/transport/mod.rs`
- `crates/astrbot-mcp/src/bridge/mod.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-066.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.summaries/TASK-066-summary.md`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`
- `.workflow/.csv-wave/20260517-execute-scratch-task-066/wave-1.csv`

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-mcp`
- `cargo test -p astrbot-tool`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all --check`
