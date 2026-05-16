# WebChat HTTP Server Context

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_adapter.py`
  - External WebChat input belongs at the platform boundary.
  - HTTP is just one transport for getting data to that boundary.

## Rust Increment

- `astrbot-web` now provides:
  - `webchat_router()`
  - `serve_webchat()`
  - `serve_webchat_with_shutdown()`
- The server boundary is thin:
  - route validation
  - delegate to `WebChatPlatform::submit_text()`
  - surface typed HTTP errors
- Added a real HTTP round-trip test using a bound TCP listener and `reqwest`.

## Result

- `cargo test -p astrbot-web` covers both router-only and real HTTP server paths.
- Workspace verification passed:
  - `cargo fmt --all --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo run -p astrbot-cli`

## Next

- Mount this server boundary into a dashboard or standalone service binary.
- Add auth/session management when that surface becomes user-facing.
