# TASK-025 Summary - Agent Runner Boundary

## Outcome

Introduced `astrbot-agent` as the typed boundary between pipeline stages and provider/tool orchestration.

New modules:

- `runner.rs`: `AgentRunner`, `AgentRunOutcome`, and `ChatAgentRunner`.
- `fallback.rs`: `AgentFallbackPolicy`.
- `request_decorator.rs`: provider request envelope building and decorator traits.
- `persona.rs`: `AgentPersona` and `PersonaPromptDecorator`.
- `tool_loop.rs`: tool-loop policy/state/outcome placeholders.

## Integration

- `ProviderStage` now delegates provider fallback to `ChatAgentRunner`.
- Pipeline-specific provider preference, session context, and quote context ports are adapted into agent request decorators.
- Existing `ProcessStage` behavior remains unchanged: plugin handlers run first, then provider fallback is delegated through the agent boundary.

## AstrBot Reference

Compared against:

- `E:/Playground/Astrbot/astrbot/core/astr_main_agent.py`
- `E:/Playground/Astrbot/astrbot/core/agent/runners/tool_loop_agent_runner.py`
- `E:/Playground/Astrbot/astrbot/core/astr_agent_tool_exec.py`

Rust now has a clear destination for persona/context/multimodal/tool-loop/subagent growth without moving those concerns into pipeline stages or provider adapters.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-agent`
- `cargo test -p astrbot-pipeline`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`
