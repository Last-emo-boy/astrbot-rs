# TASK-053 Summary

## Result

Added `crates/astrbot-provider/src/model_resolver` with a shared `XinferenceModelResolver` and typed `XinferenceModelType`. The resolver now owns model UID caching, `GET /v1/models` lookup, optional `POST /v1/models` launch, launch model type, and the not-running/auto-launch-disabled error policy.

`XinferenceSpeechToTextProvider` and `XinferenceRerankProvider` now delegate model lifecycle resolution to the shared resolver. The adapters keep their local responsibilities: HTTP client/header setup, audio loading or rerank payload building, transcription/rerank dispatch, and response parsing.

## Files

- `crates/astrbot-provider/src/lib.rs`
- `crates/astrbot-provider/src/model_resolver/mod.rs`
- `crates/astrbot-provider/src/model_resolver/xinference.rs`
- `crates/astrbot-provider/src/xinference_stt.rs`
- `crates/astrbot-provider/src/xinference_rerank.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-053.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.summaries/TASK-053-summary.md`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`
- `.workflow/.csv-wave/20260517-execute-scratch-task-053/wave-1.csv`

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-provider --test xinference_stt --test xinference_rerank`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo fmt --all --check`
