# WebChat Platform Context

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_adapter.py`
  - WebChat receives external user payloads through a queue/listener.
  - It converts the payload to a platform message event and commits it to the event queue.
  - Replies are delivered through the platform event's send path.

## Rust Increment

- Added `WEBCHAT_PLATFORM_TYPE`.
- Added `PlatformConfig::webchat()` and `RuntimePlatformConfig::webchat()`.
- Added `WebChatPlatform`:
  - `submit_text(conversation_id, sender_id, text)` creates a unified `MessageEvent`
  - replies use a `RecordingSink`, making this entrypoint testable and usable by a future dashboard/HTTP layer
- Added `PlatformManager::webchat_platform(id)` accessor.
- Registered `webchat` in `PlatformRegistry::with_builtin_platforms()`.

## Result

- Platform tests cover webchat registration, manager construction and text submission to event queue.
- Runtime test covers WebChat input through runtime/pipeline/provider/respond into recorded reply output.
- Verification passed:
  - `cargo fmt --all --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo run -p astrbot-cli`

## Next

- Add an HTTP/dashboard layer that calls `WebChatPlatform::submit_text()`.
- Decide persistence/history behavior for WebChat conversations.
