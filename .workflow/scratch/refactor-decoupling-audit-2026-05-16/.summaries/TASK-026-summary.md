# TASK-026 Summary - Management API Boundary

## Outcome

Separated dashboard-facing management APIs from WebChat chat transport routes.

New modules:

- `management/mod.rs`: management API router and shared state.
- `management/status.rs`: runtime/provider/platform/plugin status snapshot DTO.
- `management/providers.rs`: provider manager snapshot DTO.
- `management/platforms.rs`: platform manager snapshot DTO.
- `management/plugins.rs`: plugin registry/handler snapshot DTO.

## Integration

- `serve_management` and `serve_management_with_shutdown` expose the management router independently from WebChat transport routes.
- `astrbot-web` re-exports management API types and helpers.
- `PlatformManager` exposes count/list helpers needed by management DTOs without exposing internal adapter storage.
- Management tests verify provider, platform, and plugin facade reads through the new router.

## AstrBot Reference

Compared against:

- `E:/Playground/Astrbot/dashboard`
- `E:/Playground/Astrbot/astrbot/core/provider/manager.py`
- `E:/Playground/Astrbot/astrbot/core/platform/manager.py`
- `E:/Playground/Astrbot/astrbot/core/star/star_manager.py`

Rust now keeps dashboard/status APIs in a dedicated HTTP namespace while WebChat submit/history routes remain transport-specific.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-web`
- `cargo test -p astrbot-platform`
- `cargo test -p astrbot-runtime`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`
