# OpenAI-Compatible Provider Context

## Maestro Task

Continue the Rust rewrite by implementing the first real chat provider boundary.

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/provider/provider.py`
  - `Provider` exposes `text_chat`, `text_chat_stream`, `get_models`, key handling and health tests.
  - Rust keeps the narrower `ChatProvider` trait for now, then adds streaming/tools later.
- `E:/Playground/Astrbot/astrbot/core/provider/entities.py`
  - `ProviderRequest` assembles OpenAI-style user messages and keeps session context explicit.
  - Rust `ChatRequest` currently keeps prompt + session_id; richer context comes later.
- `E:/Playground/Astrbot/astrbot/core/provider/sources/openai_source.py`
  - OpenAI-compatible provider supports custom base URL, model, API key, custom headers, response normalization and error truncation.

## Current Increment

- Add `OpenAiCompatibleProvider` behind the existing `ChatProvider` trait.
- Support non-streaming `/chat/completions`.
- Normalize assistant content from string, `{text: ...}` object, or OpenAI content parts array.
- Keep tests local with a mock TCP HTTP server; no real external API calls.
- Leave streaming, tools, multimodal inputs, key rotation and model listing for later.

## Result

- Added `OpenAiCompatibleProvider` and `OpenAiCompatibleConfig` in `astrbot-provider`.
- Kept HTTP/JSON dependencies inside `astrbot-provider`; `astrbot-core` remains transport-agnostic.
- Added local TCP HTTP mock tests for request shape, string content, content parts and error mapping.
- CLI still uses `MockChatProvider` by default; setting `ASTRBOT_OPENAI_API_KEY` switches to OpenAI-compatible provider with optional `ASTRBOT_OPENAI_API_BASE`, `ASTRBOT_OPENAI_MODEL`, `ASTRBOT_OPENAI_TIMEOUT_SECS`.
- Verification passed: `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, CLI smoke.
