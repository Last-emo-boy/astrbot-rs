# TASK-010 Summary

Status: completed

Prepared platform adapter-local module trees for OneBot and WebChat before WeChat/QQ parity expands.

Changes:

- `adapters/onebot/mod.rs` keeps `OneBotPlatform` as the public adapter type.
- `adapters/onebot/event.rs` owns unified `MessageEvent` construction.
- `adapters/onebot/message.rs` owns OneBot private/group session and text message conversion helpers.
- `adapters/webchat/mod.rs` keeps `WebChatPlatform` as the public adapter type.
- `adapters/webchat/event.rs` owns WebChat `MessageEvent` construction.
- `adapters/webchat/message.rs` owns WebChat text/image message-chain construction.

Verification:

- `cargo fmt --all --check`
- `cargo test -p astrbot-platform`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
