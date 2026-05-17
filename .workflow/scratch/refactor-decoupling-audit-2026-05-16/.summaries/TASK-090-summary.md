# TASK-090 Summary

Completed at: 2026-05-17T17:01:58+08:00

## Scope

Split the remaining broad agent, MCP, and tool crate test facades by behavior boundary without changing production behavior.

## Changes

- Replaced `crates/astrbot-agent/src/tests.rs` with `tests/mod.rs`, `support.rs`, `request_decorator.rs`, `context.rs`, `runner.rs`, `message.rs`, and `tool_loop.rs`.
- Replaced `crates/astrbot-mcp/src/tests.rs` with focused modules for bridge, config, elicitation, prompts/sampling, resources, roots, and tools.
- Replaced `crates/astrbot-tool/src/tests.rs` with focused modules for catalog, schema, commands, conflicts, source/activation, internal providers, and references.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-agent`
- `cargo test -p astrbot-mcp`
- `cargo test -p astrbot-tool`
- `cargo clippy --workspace -- -D warnings`

## Result

All 90 tasks in the refactor decoupling audit are complete.
