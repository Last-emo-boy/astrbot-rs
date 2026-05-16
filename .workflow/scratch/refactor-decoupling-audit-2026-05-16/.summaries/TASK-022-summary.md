# TASK-022 Summary - Provider Protocol Boundary

Status: completed

## Scope

Extract provider protocol DTOs, content-part conversion, response parsing, and SSE parsing from concrete adapter orchestration files.

## Changes

- Added `protocol/sse.rs` for shared SSE `data:` line extraction.
- Added `protocol/openai_chat.rs` for OpenAI-compatible chat payload construction, response content extraction, and streaming chunk collection.
- Added `protocol/gemini_chat.rs` for Gemini `generateContent` payloads, data URL image conversion, finish-reason/block parsing, and tests.
- Added `protocol/anthropic_chat.rs` for Anthropic messages payloads, data URL image block conversion, response content extraction, and tests.
- Added `protocol/minimax_tts.rs` for MiniMax TTS request DTOs, SSE hex audio collection, error extraction, and tests.
- Added `protocol/xinference.rs` for Xinference list/launch model parsing, STT text parsing, rerank response parsing, and shared request DTOs.
- Updated OpenAI-compatible, Gemini, Anthropic, MiniMax TTS, Xinference STT, and Xinference Rerank adapters to call protocol helpers while retaining config, URL, header, HTTP, media loading, and trait orchestration locally.

## AstrBot Reference

This keeps the Rust adapter shape aligned with AstrBot provider source concepts in `openai_source.py`, `gemini_source.py`, `minimax_tts_api_source.py`, `xinference_stt_provider.py`, and `xinference_rerank_source.py`, but uses Rust modules to keep wire protocol logic separate from adapter lifecycle code.

## Follow-Up Audit

Two additional provider decoupling spaces were recorded:

- `TASK-052`: remaining non-chat provider protocol DTO/parser extraction for embedding, STT, TTS, and rerank adapters.
- `TASK-053`: shared Xinference model resolver lifecycle for model UID cache, list, optional launch, and disabled auto-launch policy.

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
