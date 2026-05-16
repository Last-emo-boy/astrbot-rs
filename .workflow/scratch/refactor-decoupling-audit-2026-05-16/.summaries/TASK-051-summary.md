# TASK-051 Summary

## Result

Introduced an `astrbot-agent::feedback` boundary for live agent feedback orchestration. `status.rs` models tool-call/tool-result status messages, streaming delta feedback, streaming break events, and final-chain extraction without coupling provider streaming parsers or `RespondStage` to user-facing status text. `stop_signal.rs` defines stop-signal policy and ports so user stop/cancel checks stay outside provider adapters and response delivery. `voice.rs` adds an optional live voice feedback bridge over the existing TTS provider trait, so TTS can be enabled or disabled without changing `ChatAgentRunner` execution. `AgentRunOutcome` can now carry feedback events alongside an optional final result.

## Files

- `crates/astrbot-agent/src/feedback/mod.rs`
- `crates/astrbot-agent/src/feedback/status.rs`
- `crates/astrbot-agent/src/feedback/stop_signal.rs`
- `crates/astrbot-agent/src/feedback/voice.rs`
- `crates/astrbot-agent/src/lib.rs`
- `crates/astrbot-agent/src/runner.rs`
- `crates/astrbot-agent/src/tests.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-051.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.summaries/TASK-051-summary.md`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`
- `.workflow/.csv-wave/20260517-execute-scratch-task-051/wave-1.csv`

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-agent`
- `cargo fmt --all --check`
- `cargo test -p astrbot-pipeline`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
