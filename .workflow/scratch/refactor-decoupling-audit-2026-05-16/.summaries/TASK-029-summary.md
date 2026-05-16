# TASK-029 Summary - MCP Boundary

## Outcome

Added a new `astrbot-mcp` crate and registered it in the workspace. The crate defines the MCP boundary as typed Rust modules before MCP runtime behavior is wired into plugin/tool/provider flows.

New modules:

- `config.rs`: MCP server config, active server filtering, transport resolution, client capability config.
- `types.rs`: typed server names, URIs, cursors, MIME types, JSON values/schema, pages, and MCP errors.
- `client.rs`: lifecycle port, runtime state, server capability snapshot.
- `tools.rs`: MCP tool descriptor, registration, and tool-call DTOs.
- `resources.rs`: resource/resource-template descriptors and server-scoped synthetic tool names.
- `prompts.rs`: prompt descriptors, prompt arguments/messages, and server-scoped synthetic tool names.
- `sampling.rs`: sampling request/message/content boundary and initial unsupported-mode guard.
- `elicitation.rs`: form/URL elicitation request types, field types, and action parsing.
- `roots.rs`: explicit roots allowlist policy and root URI model.
- `naming.rs`: shared server-name sanitizer for bridge tool names.

## Integration

- `Cargo.toml` now includes `crates/astrbot-mcp`.
- `astrbot-mcp` depends only on shared workspace crates and does not make `astrbot-plugin`, providers, or pipeline own MCP lifecycle.
- Existing `PluginToolKind::Mcp` remains a lightweight declaration; full sandbox/SDK integration is left behind this typed boundary.

## AstrBot Reference

Compared against:

- `E:/Playground/Astrbot/astrbot/core/agent/mcp_client.py`
- `E:/Playground/Astrbot/astrbot/core/agent/mcp_subcapability_bridge.py`
- `E:/Playground/Astrbot/astrbot/core/agent/mcp_resource_bridge.py`
- `E:/Playground/Astrbot/astrbot/core/agent/mcp_prompt_bridge.py`
- `E:/Playground/Astrbot/astrbot/core/provider/func_tool_manager.py`

Rust preserves AstrBot's separation between MCP client lifecycle, tool bridging, resource/prompt synthetic tools, sampling, elicitation, and roots, but models them as typed capability modules instead of mixing them into a function tool manager.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-mcp`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`
