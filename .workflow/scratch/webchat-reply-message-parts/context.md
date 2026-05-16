# WebChat Reply Message Parts Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/message_parts_helper.py`
  - Reply parts are converted into `Reply` components.
  - Reply-only messages are rejected in strict mode as empty content.
- `E:/Playground/Astrbot/astrbot/core/message/components.py`
  - `Reply` stores the referenced message ID and selected/derived message text.

## Current Rust State

- Core message components support plain plus image/record/video/file media.
- WebChat typed parts preserve submit/readback for plain and media.
- There is no reply component yet.

## Decision

Add a lightweight `Reply` component with `message_id` and `selected_text`. A reply component alone should not count as message content, but reply plus plain/media content should be preserved through submit and readback.

## Completed

- `MessageComponent::Reply` stores `message_id` and `selected_text`.
- Reply-only chains remain empty content and WebChat submit returns `EmptyMessage`.
- WebChat submit accepts reply parts, including integer `message_id` values that are normalized to strings.
- WebChat history readback serializes reply parts alongside plain/media parts.
- Pipeline coverage keeps reply/non-image-only messages from calling chat providers.

## Verification Results

- `cargo test -p astrbot-core`
- `cargo test -p astrbot-web`
- `cargo test -p astrbot-pipeline`
- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
