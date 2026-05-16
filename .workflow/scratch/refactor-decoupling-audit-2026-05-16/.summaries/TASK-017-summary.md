# TASK-017 Summary

## Scope

Split `crates/astrbot-runtime/src/handle.rs` into a focused `handle/` module tree while preserving the public `AstrbotRuntime` and `RuntimeHandle` exports.

## Result

- `handle/mod.rs`: facade re-export for runtime handle types.
- `handle/runtime.rs`: `AstrbotRuntime`, `RuntimeHandle`, initialization, accessors, start/stop, and public mock/sent-message helpers.
- `handle/supervisor.rs`: background event-bus/platform task spawning and stop/join error mapping.
- `handle/restart.rs`: restart state capture and provider preference restore policy.
- `handle/testing.rs`: shared mock platform event emission and sent-message readback helpers.

The split follows AstrBot lifecycle/platform references by keeping background task supervision separate from restart state transfer and manager termination.

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-runtime`
- `cargo clippy -p astrbot-runtime -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
