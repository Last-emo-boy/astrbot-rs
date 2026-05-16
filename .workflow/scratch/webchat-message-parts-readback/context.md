# WebChat Message Parts Readback Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/message_parts_helper.py`
  - `message_chain_to_storage_message_parts()` serializes reply chains back into typed WebChat parts.

## Current Rust State

- HTTP submit accepts typed `plain` and `image` parts.
- WebChat history currently returns only `text`, which drops image components from recorded replies.

## Decision

Extend WebChat history responses with `image_urls` and `message_parts`, using the same currently supported `plain` and `image` part shape. Keep `text` for compatibility.

## Completed

- `WebChatMessageResponse` now includes `text`, `image_urls`, and typed `message_parts`.
- WebChat history serializes recorded `MessageChain` values into currently supported `plain` and `image` parts.
- Existing runtime readback keeps the compatibility `text` field while also returning typed plain parts.

## Verification Results

- `cargo test -p astrbot-web`
- `cargo test -p astrbot-cli`
- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
