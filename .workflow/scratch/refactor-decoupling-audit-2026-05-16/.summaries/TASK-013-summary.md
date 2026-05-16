# TASK-013 Summary

Status: completed

Split the pipeline stage registry into focused modules while preserving public re-exports and scheduler behavior.

Changes:

- `registry.rs` remains the public facade and core registration API.
- `registry/order.rs` owns stage type and order constants.
- `registry/builtins.rs` owns default built-in stage registration.
- `registry/entry.rs` owns the registered stage entry and factory wrapper.
- Registry tests are grouped under `registry/tests/` by registration, order, builtins, and scheduler behavior.

AstrBot alignment:

- Mirrors AstrBot's split between `stage_order.py`, `bootstrap.py`, and `scheduler.py`.
- Keeps Rust's registry deterministic and typed instead of relying on Python import side effects.
- Preserves `DefaultPipelineBuilder` and `PipelineScheduler` behavior.

Verification:

- `cargo fmt --all --check`
- `cargo test -p astrbot-pipeline`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
