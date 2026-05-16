# WebChat Message Parts Submit Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/message_parts_helper.py`
  - WebChat accepts a string payload or a list of typed message parts.
  - Plain and media parts are normalized into message components before the event enters the pipeline.

## Current Rust State

- `SubmitTextRequest` accepts `text` plus `image_urls`.
- `WebChatPlatform::submit_message()` builds text and image URL components before sending a unified `MessageEvent`.

## Decision

Add a typed `message_parts` field to the HTTP submit request for the currently supported component set: `plain` and `image`. Keep legacy `text` and `image_urls` fields working.

The platform layer should expose a `submit_chain()` boundary so HTTP can submit already-normalized WebChat parts without directly constructing events.

## Completed

- `WebChatPlatform::submit_chain()` accepts an already-normalized `MessageChain` and rejects empty chains.
- `SubmitTextRequest` now accepts defaulted `message_parts` in addition to legacy `text` and `image_urls`.
- HTTP submit maps typed `plain` and `image` parts into `MessageChain`, then calls the platform boundary.

## Verification Results

- `cargo test -p astrbot-platform`
- `cargo test -p astrbot-web`
- `cargo test -p astrbot-cli`
- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
