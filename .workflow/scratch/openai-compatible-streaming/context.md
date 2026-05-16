# OpenAI-Compatible Streaming Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/provider/provider.py`
  - Provider interface separates `text_chat()` and `text_chat_stream()`.
- `E:/Playground/Astrbot/astrbot/core/provider/sources/openai_source.py`
  - OpenAI provider sends `stream=True`, yields chunks while preserving whitespace, then emits a final complete response.
- `E:/Playground/Astrbot/astrbot/core/astr_main_agent.py`
  - Runtime chooses whether streaming is enabled and may disable it for unsupported WebChat output modalities.

## Current Rust State

- `ChatProvider::chat()` currently returns one `ChatResponse`.
- `OpenAiCompatibleProvider` always sends `stream: false`.
- The pipeline can stay non-streaming for now; provider-level streaming can first collect SSE chunks into the final `ChatResponse`.

## Decision

Add an opt-in `ChatRequest::with_stream(true)` flag. The OpenAI-compatible provider sends `stream: true` and parses OpenAI-style SSE `data:` events into a final text response. This preserves the existing pipeline contract while laying the parsing groundwork for future real-time WebChat streaming.

## Verification

- Existing non-streaming tests must keep passing and keep asserting `stream:false`.
- New provider test should feed OpenAI-style SSE chunks and assert the final accumulated text and request payload `stream:true`.

## Execution Notes

- `ChatRequest` now carries `stream: bool` with a default of `false`.
- The OpenAI-compatible provider collects SSE `data:` chunks into the final text response when streaming is enabled.
- Non-streaming JSON responses and existing provider registry behavior remain unchanged.
- Verified with `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, and `cargo run -p astrbot-cli`.
