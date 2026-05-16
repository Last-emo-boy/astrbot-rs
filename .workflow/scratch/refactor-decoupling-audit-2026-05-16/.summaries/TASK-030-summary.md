# TASK-030 Summary - Tool Schema And Command Boundary

## Outcome

Added a new `astrbot-tool` crate and registered it in the workspace. The crate separates tool catalog state, provider-specific schema serialization, activation policy, command descriptors, and conflict detection before tool-call parity reaches providers or plugin registry internals.

New modules:

- `catalog.rs`: `ToolDescriptor`, `ToolCatalog`, tool source, deterministic replacement and active tool views.
- `activation.rs`: disabled-tool and rename policy independent from runtime/plugin registry mutation.
- `commands.rs`: command/group/sub-command descriptors, permissions, parent command composition, aliases.
- `conflicts.rs`: enabled command and active tool conflict detection.
- `schema/openai.rs`: OpenAI function-tool schema serializer.
- `schema/anthropic.rs`: Anthropic tool schema serializer.
- `schema/gemini.rs`: Gemini function declaration serializer and JSON schema normalization.

## Integration

- `Cargo.toml` now includes `crates/astrbot-tool`.
- Provider adapters remain unchanged and can later consume already-shaped schemas instead of owning catalog/command policy.
- `ProviderRequest` remains lightweight with tool placeholders and call results; full command state lives outside the request DTO.

## AstrBot Reference

Compared against:

- `E:/Playground/Astrbot/astrbot/core/provider/func_tool_manager.py`
- `E:/Playground/Astrbot/astrbot/core/agent/tool.py`
- `E:/Playground/Astrbot/astrbot/core/star/command_management.py`
- `E:/Playground/Astrbot/astrbot/core/star/register/star_handler.py`

Rust keeps AstrBot's ToolSet schema formats and command-management concepts, but moves them into typed modules instead of coupling them to one function tool manager or star handler registry.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-tool`
- `cargo test -p astrbot-plugin`
- `cargo test -p astrbot-provider`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`
