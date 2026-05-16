# TASK-057 Summary

## Result

Replaced the monolithic `crates/astrbot-provider/src/factories/tts.rs` with a `factories/tts` module directory. The new `mod.rs` keeps the existing builder exports stable for the registry, while provider-specific construction moved into:

- `openai.rs`
- `gemini.rs`
- `volcengine.rs`
- `minimax.rs`
- `gsvi.rs`

Added `options.rs` as the TTS factory option-normalization boundary. It centralizes provider option alias lookup plus TTS-specific bool, float, JSON, and named numeric parse helpers while preserving existing error messages. Protocol DTO/parsing and media artifact writing remain in their existing provider/protocol modules.

## Files

- `crates/astrbot-provider/src/factories/tts.rs`
- `crates/astrbot-provider/src/factories/tts/mod.rs`
- `crates/astrbot-provider/src/factories/tts/options.rs`
- `crates/astrbot-provider/src/factories/tts/openai.rs`
- `crates/astrbot-provider/src/factories/tts/gemini.rs`
- `crates/astrbot-provider/src/factories/tts/volcengine.rs`
- `crates/astrbot-provider/src/factories/tts/minimax.rs`
- `crates/astrbot-provider/src/factories/tts/gsvi.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-057.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.summaries/TASK-057-summary.md`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`
- `.workflow/.csv-wave/20260517-execute-scratch-task-057/wave-1.csv`

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo fmt --all --check`
