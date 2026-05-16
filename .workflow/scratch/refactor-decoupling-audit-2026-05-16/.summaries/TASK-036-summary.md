# TASK-036 Summary

Completed: 2026-05-16T19:34:20+08:00

Scope:
- Extract provider-neutral streaming helpers from concrete provider protocol modules.
- Keep protocol-specific DTO and response parsing inside each provider protocol module.
- Preserve the core/pipeline `MessageStream` delivery boundary without adding provider transport parsing there.

Changes:
- Added `crates/astrbot-provider/src/streaming/{mod.rs,sse.rs,chunk.rs,policy.rs}`.
- Moved generic SSE `data:` line extraction into `streaming::sse`.
- Moved streamed text delta normalization into `streaming::chunk`.
- Moved unsupported streaming rejection into `streaming::policy` while preserving existing provider error text.
- Updated OpenAI-compatible chat and MiniMax TTS protocol parsers to use shared SSE helpers.
- Updated Anthropic and Gemini providers to use the shared unsupported streaming policy.
- Removed the protocol-local SSE helper module.

AstrBot reference:
- `provider_settings.streaming_response` and `unsupported_streaming_strategy` from `core/config/default.py`.
- `text_chat_stream`, TTS `support_stream`, and `get_audio_stream` from `core/provider/provider.py`.
- streaming/fallback behavior from `core/agent/runners/tool_loop_agent_runner.py`.

Verification:
- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test -p astrbot-pipeline`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next:
- `M7-R32-multimodal-preparation-boundary` / `TASK-037`.
