# TASK-015 CLI Entrypoint Boundary

`TASK-015` is complete. `astrbot-cli` now uses a thin `main.rs` entrypoint that wires argument parsing into command execution.

Implementation modules:

- `args.rs`: `CliCommand`, default config path, and parse helpers.
- `commands/mod.rs`: command dispatch facade.
- `commands/init.rs`: config initialization command.
- `commands/run.rs`: runtime startup and shutdown flow.
- `commands/smoke.rs`: mock message smoke command.
- `webchat_server.rs`: WebChat server preparation, start, address, and shutdown handle.
- `tests.rs`: CLI parsing and WebChat server integration tests.

AstrBot comparison:

- Keeps the Rust CLI as a thin lifecycle boundary instead of copying AstrBot's broad lifecycle/dashboard startup coupling.
- Preserves runtime-created platform handles for WebChat HTTP server launch, so CLI does not reach into pipeline/event internals.

Verification passed:

- `cargo test -p astrbot-cli`
- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
