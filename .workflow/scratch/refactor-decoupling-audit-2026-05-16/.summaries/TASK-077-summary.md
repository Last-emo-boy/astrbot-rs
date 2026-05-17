# TASK-077 Summary

Completed at: 2026-05-17T14:04:14+08:00

## Scope

Defined agent lifecycle side-channel boundaries for hooks, run context, response events, message wrappers, and tool-image cache behavior.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/agent/hooks.py`
- `E:/Playground/Astrbot/astrbot/core/agent/run_context.py`
- `E:/Playground/Astrbot/astrbot/core/agent/message.py`
- `E:/Playground/Astrbot/astrbot/core/agent/response.py`
- `E:/Playground/Astrbot/astrbot/core/agent/tool_image_cache.py`
- `E:/Playground/Astrbot/astrbot/core/astr_agent_hooks.py`

## Changes

- Added `AgentRunContext`, `AgentMessage`, `AgentResponseEvent`, `AgentRunHook`, and tool-image cache ports under `astrbot-agent`.
- Added `ChatAgentRunner::with_hook` and dispatches `AgentBegin` / `AgentDone` around provider fallback without moving hook internals into the runner.
- Added `PipelineContext::with_agent_run_hook` so pipeline stages pass a typed hook port into the agent instead of embedding agent side effects.
- Added tests for message/tool-call wrappers, run-context timeout/state, response stats mapping, in-memory tool-image cache, runner hook dispatch, and pipeline hook-port routing.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-agent`
- `cargo test -p astrbot-pipeline`
- `cargo clippy --workspace -- -D warnings`

## Next

Next pending task is `TASK-078`.
