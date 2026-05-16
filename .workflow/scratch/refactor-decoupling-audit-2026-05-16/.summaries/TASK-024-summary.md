# TASK-024 Summary - Storage Repository Boundary

## Outcome

Introduced `astrbot-storage` as the storage port crate for low-level persistence contracts.

Implemented in-memory repositories for:

- conversation history
- attachments
- provider preferences
- config snapshots

## Integration

- `RecordingSink` now implements `ConversationHistoryRepository`.
- `WebChatPlatform` exposes `conversation_history()` as a storage trait object.
- WebChat history routes read `ConversationMessageRecord` values and map storage errors.
- `InMemoryProviderPreferencePort` now delegates to `ProviderPreferenceRepository`.
- Runtime restart state capture/restore awaits provider preference snapshot replacement.

## AstrBot Reference

Compared against:

- `E:/Playground/Astrbot/astrbot/core/db/sqlite.py`
- `E:/Playground/Astrbot/astrbot/core/backup/exporter.py`
- `E:/Playground/Astrbot/astrbot/core/backup/importer.py`

The Rust implementation keeps persistent backend work deferred, but platform, web, pipeline, and runtime code no longer need to own storage details.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-storage`
- `cargo test -p astrbot-pipeline`
- `cargo test -p astrbot-platform`
- `cargo test -p astrbot-web`
- `cargo test -p astrbot-runtime`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`
