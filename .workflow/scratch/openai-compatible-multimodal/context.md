# OpenAI-Compatible Multimodal Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/provider/provider.py`
  - Provider text chat APIs accept `image_urls`.
- `E:/Playground/Astrbot/astrbot/core/provider/entities.py`
  - `ProviderRequest.assemble_context()` converts prompt + image URLs into OpenAI content parts.
  - It keeps simple text as a plain string when no images/extra parts exist.
- `E:/Playground/Astrbot/astrbot/core/provider/sources/openai_source.py`
  - OpenAI provider prepares payloads from that unified request representation.

## Current Rust State

- `ChatRequest` has prompt/session/stream but no image input.
- `OpenAiCompatibleProvider` always serializes user content as a plain string.
- Pipeline can keep sending text-only requests; multimodal support should be opt-in through request builders.

## Decision

Add `image_urls` to `ChatRequest` and make OpenAI-compatible payload construction switch from string content to content parts only when images are present. This mirrors AstrBot's backward-compatible behavior: text-only requests remain simple strings, multimodal requests become `[{type:text}, {type:image_url}]`.

## Verification

- Existing text-only request test keeps asserting `"content":"hello"`.
- New test asserts multimodal request payload contains text and image URL parts.

## Execution Notes

- `ChatRequest` now carries `image_urls` and has builder helpers for single or multiple image URLs.
- Text-only OpenAI-compatible requests still serialize `content` as a string.
- Requests with images serialize `content` as OpenAI content parts with one text part and image URL parts.
- Verified with `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, and `cargo run -p astrbot-cli`.
