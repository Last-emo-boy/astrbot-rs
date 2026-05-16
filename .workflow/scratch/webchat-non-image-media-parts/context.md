# WebChat Non-Image Media Parts Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/message_parts_helper.py`
  - `MEDIA_PART_TYPES = {"image", "record", "file", "video"}`
  - WebChat converts supported media parts into unified message components before event dispatch.

## Current Rust State

- `MessageComponent` supports `Plain` and `Image`.
- WebChat submit/readback supports typed `plain` and `image` parts.
- ProviderStage only forwards text and image URLs to chat providers.

## Decision

Add `Record`, `Video`, and `File` as core message components and WebChat typed parts. Keep ProviderStage unchanged so non-image media is preserved in events/history without pretending providers can consume it yet.

## Completed

- `MessageComponent` now supports `Record`, `Video`, and `File`.
- WebChat submit accepts typed `record`, `video`, and `file` parts.
- WebChat history readback serializes non-image media parts in `message_parts`.
- Pipeline coverage proves non-image-only media does not call chat providers yet.

## Verification Results

- `cargo test -p astrbot-core`
- `cargo test -p astrbot-web`
- `cargo test -p astrbot-pipeline`
- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
