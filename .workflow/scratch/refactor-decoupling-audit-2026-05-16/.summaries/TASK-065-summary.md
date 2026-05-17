# TASK-065 Summary

## Result

Added an external agent runner boundary in `astrbot-agent` for non-standard workflow agents such as Coze, Dify, DashScope, and DeerFlow. The boundary models connector config, provider-request adaptation, run lifecycle state, raw stream events, streaming delta mapping, final result mapping, and connector lifecycle traits outside normal chat provider adapters.

Added runtime external runner config mapping through `RuntimeExternalAgentConfig` and `external_agent_runners`, converting to `ExternalAgentConnectorConfig` without routing these entries through `ChatProviderConfig`.

## Files

- `Cargo.lock`
- `crates/astrbot-agent/src/lib.rs`
- `crates/astrbot-agent/src/external/mod.rs`
- `crates/astrbot-runtime/Cargo.toml`
- `crates/astrbot-runtime/src/config.rs`
- `crates/astrbot-runtime/src/lib.rs`
- `crates/astrbot-runtime/src/provider_config.rs`
- `crates/astrbot-runtime/src/provider_config/external_agent.rs`
- `crates/astrbot-runtime/src/tests/provider_config.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-065.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.summaries/TASK-065-summary.md`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`
- `.workflow/.csv-wave/20260517-execute-scratch-task-065/wave-1.csv`

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-agent`
- `cargo test -p astrbot-runtime`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all --check`
