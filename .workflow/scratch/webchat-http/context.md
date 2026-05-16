# WebChat HTTP Boundary Context

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_adapter.py`
  - WebChat adapter is the boundary from external input into AstrBot's event system.
  - It does not own pipeline logic; it just translates external data into an event and commits it.

## Rust Increment

- Added new crate `astrbot-web`.
- `webchat_router()` exposes:
  - `POST /api/webchat/{conversation_id}`
  - request body `{ sender_id, text }`
  - response `{ event_id }`
- Router delegates input handling to `WebChatPlatform::submit_text()`.
- Errors are mapped to HTTP status codes without exposing pipeline internals.

## Result

- Route tests confirm valid requests produce an event and empty text returns `400 Bad Request`.
- Workspace verification passed:
  - `cargo fmt --all --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo run -p astrbot-cli`

## Next

- Mount this router into an actual HTTP server or dashboard crate.
- Add auth/session management before exposing it publicly.
