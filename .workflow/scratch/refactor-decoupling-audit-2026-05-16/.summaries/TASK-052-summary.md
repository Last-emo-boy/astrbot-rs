# TASK-052 Summary

## Result

Moved non-chat provider protocol DTOs and response parsers under `crates/astrbot-provider/src/protocol`. OpenAI and Gemini embedding request/response mapping now live in dedicated protocol modules. Shared TTS protocol mapping covers OpenAI TTS payloads, Gemini TTS payload/response/WAV conversion, and Volcengine TTS payload/audio decoding while leaving artifact writing in concrete adapters. Bailian and VLLM rerank request/response mapping now lives in `protocol/rerank.rs`. OpenAI STT multipart form construction and transcription response parsing were also moved to `protocol/speech.rs` to satisfy the STT convergence criterion.

Concrete adapters now mostly orchestrate config, HTTP clients, request dispatch, provider error mapping, media loading, artifact writing, and trait implementation.

## Files

- `crates/astrbot-provider/src/protocol/mod.rs`
- `crates/astrbot-provider/src/protocol/openai_embedding.rs`
- `crates/astrbot-provider/src/protocol/gemini_embedding.rs`
- `crates/astrbot-provider/src/protocol/tts.rs`
- `crates/astrbot-provider/src/protocol/rerank.rs`
- `crates/astrbot-provider/src/protocol/speech.rs`
- `crates/astrbot-provider/src/openai_embedding.rs`
- `crates/astrbot-provider/src/gemini_embedding.rs`
- `crates/astrbot-provider/src/openai_tts.rs`
- `crates/astrbot-provider/src/gemini_tts.rs`
- `crates/astrbot-provider/src/volcengine_tts.rs`
- `crates/astrbot-provider/src/openai_stt.rs`
- `crates/astrbot-provider/src/bailian_rerank.rs`
- `crates/astrbot-provider/src/vllm_rerank.rs`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.task/TASK-052.json`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/.summaries/TASK-052-summary.md`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/index.json`
- `.workflow/.csv-wave/20260517-execute-scratch-task-052/wave-1.csv`

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-provider`
- `cargo fmt --all --check`
- `cargo clippy -p astrbot-provider -- -D warnings`
