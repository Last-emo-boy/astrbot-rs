# Pipeline Multimodal Request Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/provider/entities.py`
  - `ProviderRequest` keeps `prompt` and `image_urls`, then assembles OpenAI content parts only when needed.
- `E:/Playground/Astrbot/astrbot/core/provider/provider.py`
  - Provider interface accepts `image_urls` as part of normal text chat.
- `E:/Playground/Astrbot/astrbot/core/pipeline/respond/stage.py`
  - Pipeline stages stay responsible for transforming unified events, not platform-specific HTTP/provider wiring.

## Current Rust State

- `MessageChain` already has `Image { url }` components.
- `ChatRequest` now supports `image_urls`, and OpenAI-compatible payloads can serialize content parts.
- `ProviderStage` still only sends `event.message.plain_text()` to providers, so images are dropped before provider execution.

## Decision

Add `MessageChain::image_urls()` and make `ProviderStage` attach those URLs to `ChatRequest`. Image-only messages should still reach the provider; the provider payload builder supplies the text placeholder for OpenAI-compatible requests.

## Verification

- Core test covers image URL extraction from message chains.
- Pipeline test uses a capturing provider to prove images are present on `ChatRequest`.
- Existing text-only pipeline behavior remains unchanged.

## Execution Notes

- Added `MessageComponent::image()` and `MessageChain::image_urls()`.
- `ProviderStage` now skips only when both text and image URLs are empty.
- Added pipeline coverage proving an image-only event reaches the provider with `ChatRequest.image_urls`.
- Verified with `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, and `cargo run -p astrbot-cli`.
