# WebChat Image URL Submit Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/message_parts_helper.py`
  - WebChat treats plain text and media parts as valid message content.
  - Media parts are parsed into platform message components before entering the event pipeline.
- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_adapter.py`
  - WebChat adapter converts dashboard input into unified platform messages.

## Current Rust State

- `MessageChain` has `Image { url }` and `ProviderStage` forwards image URLs into `ChatRequest`.
- OpenAI-compatible provider serializes `ChatRequest.image_urls` into OpenAI content parts.
- `WebChatPlatform::submit_text()` and HTTP `SubmitTextRequest` only accept text.

## Decision

Add a backward-compatible `image_urls` field to the WebChat submit API. Text-only requests keep working. Image-only requests are valid when at least one non-empty image URL is present.

## Verification

- Platform tests cover image-only submit.
- HTTP route tests cover text + image URL submit and image-only submit.
- Existing CLI/runtime WebChat tests are updated to include empty `image_urls`.

## Completed

- `WebChatPlatform::submit_message()` builds `MessageChain` from optional text and non-empty image URLs.
- `SubmitTextRequest` now accepts defaulted `image_urls` and the Axum route delegates to the platform boundary.
- Image-only WebChat HTTP input is accepted when it contains at least one non-empty image URL.

## Verification Results

- `cargo test -p astrbot-platform`
- `cargo test -p astrbot-web`
- `cargo test -p astrbot-cli`
- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
