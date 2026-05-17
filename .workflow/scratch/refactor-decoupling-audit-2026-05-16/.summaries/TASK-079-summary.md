# TASK-079 Summary

Completed at: 2026-05-17T14:30:08+08:00

## Scope

Introduced platform identity, member profile, group metadata, and permission resolution boundaries without changing `MessageSession` into membership state.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/platform/astrbot_message.py`
- `E:/Playground/Astrbot/astrbot/core/platform/astr_message_event.py`
- `E:/Playground/Astrbot/astrbot/core/platform/message_session.py`
- `E:/Playground/Astrbot/astrbot/core/platform/sources/aiocqhttp/aiocqhttp_platform_adapter.py`
- `E:/Playground/Astrbot/astrbot/core/platform/sources/aiocqhttp/aiocqhttp_message_event.py`
- `E:/Playground/Astrbot/astrbot/core/platform/sources/satori/satori_adapter.py`

## Changes

- Added core `PlatformMemberRole`, `PlatformMemberProfile`, `PlatformGroupMetadata`, and `PlatformIdentity` models under `astrbot-core/src/message/identity.rs`.
- Extended `MessageEvent` with optional normalized identity metadata while preserving `MessageSender` and `MessageSession` routing semantics.
- Added platform common `PlatformIdentityNormalizer` and `PlatformGroupIdentityInput` for adapter-owned sender/group normalization.
- Added platform common `PlatformPermissionResolver` and `IdentityPermissionResolver` for typed member/admin/owner resolution from event identity.
- Updated console, mock, webchat, and onebot event builders to attach normalized identity; onebot group events expose normalized group metadata from the routing session.
- Updated plugin permission filtering to resolve permissions from event identity plus the existing static scope fallback.
- Added tests for session/identity separation, identity normalization, typed permission resolution, and static scope compatibility.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-core`
- `cargo test -p astrbot-platform`
- `cargo test -p astrbot-plugin`
- `cargo clippy --workspace -- -D warnings`

## Next

Next pending task is `TASK-080`.
