# TASK-033 Summary

`TASK-033` is completed. `astrbot-platform` now has common typed boundaries for webhook callback security, callback server contracts, long-connection lifecycle, and per-route queue state before real WeChat/QQ/WeCom adapters are expanded.

Implemented boundaries:

- `adapters/common/security.rs`: signature input/verdict/verifier traits, SHA1 sorted-field verifier inspired by WeCom verification, encrypted-envelope DTOs, decoded payload DTO, and payload codec trait.
- `adapters/common/webhook.rs`: HTTP callback request/response DTOs, endpoint/method declarations, callback handler/server traits, server state, and retry event deduplication.
- `adapters/common/long_connection.rs`: endpoint/state/reconnect policy, command/frame DTOs, request waiters, and long-connection client trait.
- `adapters/common/queue.rs`: inbound/outbound callback queue items, pending webhook responses, stats, queue trait, and in-memory store.
- `adapters/common/transport.rs`: added `LongConnection` transport kind so future adapters can report long-lived platform connections without leaking details into `PlatformManager`.

AstrBot references used:

- `E:/Playground/Astrbot/astrbot/core/platform/sources/qqofficial_webhook/qo_webhook_server.py`
- `E:/Playground/Astrbot/astrbot/core/platform/sources/qqofficial_webhook/qo_webhook_adapter.py`
- `E:/Playground/Astrbot/astrbot/core/platform/sources/wecom_ai_bot/wecomai_server.py`
- `E:/Playground/Astrbot/astrbot/core/platform/sources/wecom_ai_bot/WXBizJsonMsgCrypt.py`
- `E:/Playground/Astrbot/astrbot/core/platform/sources/wecom_ai_bot/wecomai_long_connection.py`
- `E:/Playground/Astrbot/astrbot/core/platform/sources/wecom_ai_bot/wecomai_queue_mgr.py`

Rust design notes:

- Common modules expose traits and DTOs only; real Axum/Quart-style route handlers, AES/Ed25519 codecs, and platform-specific event conversion remain outside the common boundary.
- Adapter event/message conversion stays in adapter-local modules. Webhook validation, deduplication, long-connection reconnect policy, and callback queues no longer need to grow inside `PlatformManager`.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-platform`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

Next concrete task: `M7-R29-provider-test-media-fixture-boundary` / `TASK-034`.
