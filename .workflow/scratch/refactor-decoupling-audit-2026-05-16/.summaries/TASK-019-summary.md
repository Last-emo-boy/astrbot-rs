# TASK-019 Summary - Tool Execution Sandbox Boundary

Status: completed

## Scope

Split tool execution, handoff, background task, and sandbox capability concepts out of the plugin SDK/sandbox surface before implementing concrete local/MCP/tool-call orchestration.

## Changes

- Added `crates/astrbot-plugin/src/tool/` as a focused tool facade.
- Added typed declarations for local, MCP, handoff, and background tools.
- Added `HandoffToolTarget` and `BackgroundTaskPolicy` boundary types.
- Added `ToolExecutionRequest`, `ToolExecutionResult`, `ToolExecutionStatus`, `ToolExecutor`, and `SandboxedToolExecutor`.
- Added `ToolCapabilityDecision` so sandbox profile checks can report missing permissions and capabilities before execution.
- Re-exported the new tool API from `astrbot-plugin`.
- Added tests for capability rejection, allowed sandboxed execution, and handoff/background declarations.

## AstrBot Reference

This follows the warning shape in `E:/Playground/Astrbot/astrbot/core/astr_agent_tool_exec.py`, where local tools, MCP tools, handoff agents, background tasks, wake-up behavior, and sandbox checks are close together. Rust now keeps these as typed boundaries before real execution is wired.

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-plugin`
- `cargo test -p astrbot-pipeline`
- `cargo clippy -p astrbot-plugin -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
