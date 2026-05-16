# M7-T1 Real Platform Adapter: OneBot

## Reference

- AstrBot registers `aiocqhttp` through `register_platform_adapter` and loads platform implementations through the platform manager.
- AstrBot aiocqhttp maps `message_type == group` to a group session and private messages to the sender user ID, then wraps the result in a platform-specific message event.
- AstrBot sendback distinguishes group and private sessions when dispatching messages to OneBot.

## Rust Decision

- `OneBotPlatform` is the first real platform-shaped adapter in `astrbot-rs`.
- It is added only through `PlatformRegistry::with_builtin_platforms()` and `PlatformManager`, not by direct runtime construction.
- The minimal surface normalizes private/group text or already-built `MessageChain` values into unified `MessageEvent`.
- `conversation_id` uses `private:{user_id}` and `group:{group_id}` to avoid QQ user/group ID collisions, while `MessageSessionKind` preserves the direct/group semantic for policy stages.
- Outbound responses still use the existing recording sink boundary. OneBot network reverse WebSocket/HTTP transport and native CQ segment conversion are deferred.

## Verification

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
