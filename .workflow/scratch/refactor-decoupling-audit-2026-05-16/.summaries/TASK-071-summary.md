# TASK-071 Summary

Completed at: 2026-05-17T12:41:31+08:00

## Scope

Added an AstrBot-inspired internal tool provider/source boundary without wiring execution into provider or plugin managers.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/provider/func_tool_manager.py`
- `E:/Playground/Astrbot/astrbot/core/tools/kb_query.py`
- `E:/Playground/Astrbot/astrbot/dashboard/routes/tools.py`

## Changes

- Added `ToolSourceMetadata` and `ToolUserTogglePolicy` so tool descriptors can distinguish internal, plugin, MCP, and subagent origins with dashboard-facing metadata.
- Added internal tool provider descriptors for AstrBot built-ins: cron, knowledge-base query, send-message, and computer-use.
- Updated activation policy so internal tools cannot be disabled through normal user toggle paths unless their source policy explicitly allows it.
- Added runtime internal tool assembly helpers that return internal registrations/catalogs without importing dashboard or provider adapter code.
- Added dashboard tool management state and routes for listing/toggling tools while keeping provider manager internals out of the web boundary.
- Updated MCP, subagent, and plugin declaration bridges to project richer source metadata.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-tool`
- `cargo test -p astrbot-plugin`
- `cargo test -p astrbot-runtime`
- `cargo test -p astrbot-web`
- `cargo test -p astrbot-agent`
- `cargo test -p astrbot-mcp`
- `cargo test -p astrbot-computer`
- `cargo clippy --workspace -- -D warnings`

## Next

Next pending task is `TASK-072`.
