# TASK-012 Summary

Status: completed

Prepared WebChat media/history service boundaries before dashboard attachment storage grows.

Changes:

- Added `attachment.rs` with `AttachmentDescriptor`, `AttachmentService`, and a passthrough implementation for future upload/storage resolution.
- Added `history.rs` to assemble `WebChatMessagesResponse` separately from HTTP route handlers.
- Updated `routes.rs` so message history readback delegates DTO assembly to the history boundary.
- Kept existing `message_parts.rs` behavior unchanged.

Verification:

- `cargo fmt --all --check`
- `cargo test -p astrbot-web`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
