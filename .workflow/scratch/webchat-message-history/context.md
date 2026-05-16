# WebChat Message History Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/dashboard/routes/chat.py`
  - WebChat submit creates a request ID, listens on a back queue, streams response chunks, and persists bot messages.
- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_event.py`
  - WebChat replies are written back through a conversation/request queue instead of coupling dashboard routes to pipeline internals.
- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_queue_mgr.py`
  - Keeps input queues and response back queues keyed by conversation/request.

## Current Rust State

- `WebChatPlatform::submit_text()` returns an event ID and sends a `MessageEvent` into the runtime event queue.
- WebChat replies already pass through the event's `RecordingSink`, but HTTP currently has no way to read those replies.
- `astrbot-web` should keep calling platform-facing APIs only, not inspect EventBus/Pipeline/Provider internals.

## Decision

This increment adds a simple conversation history/readback endpoint before implementing full SSE/WebSocket streaming. The platform remains the boundary: `WebChatPlatform` exposes filtered sent messages for one conversation, and `astrbot-web` serializes that into a small HTTP DTO.

## Verification

- Platform tests cover filtering WebChat sent messages by conversation.
- Web tests cover `GET /api/webchat/{conversation_id}/messages`.
- A server round trip covers POST -> runtime processing -> GET response history.

## Execution Notes

- Added `WebChatPlatform::sent_messages_for_conversation()` as the HTTP-facing readback boundary.
- Added `GET /api/webchat/{conversation_id}/messages`, returning text replies already sent through the WebChat sink.
- Added router-only coverage and a runtime-backed HTTP test that posts a message, waits for runtime response, then reads it through the history endpoint.
- Verified with `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, and `cargo run -p astrbot-cli`.
