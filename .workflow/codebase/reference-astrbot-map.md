# AstrBot Reference Map

Source: `E:/playground/Astrbot`
Date: 2026-05-15

## Product Idea

AstrBot is best understood as agentic chat infrastructure:

- Many chat platforms feed messages into one runtime.
- One runtime provides personas, conversations, providers, tools, knowledge base, plugins and WebUI.
- Plugins and platform/provider adapters are first-class extension points.
- Operators manage the system through config and dashboard instead of editing code.

## Runtime Flow

1. `astrbot/core/core_lifecycle.py`
   - Wires config, database, persona, provider, platform, knowledge base, cron, plugin manager, pipeline scheduler and event bus.
   - Rust target: split wiring into an application crate that depends on interface crates, not the other way around.

2. `astrbot/core/platform/platform.py`
   - Defines platform adapter lifecycle and `commit_event`.
   - Rust target: `PlatformAdapter` trait owns inbound transport and submits typed `MessageEvent`.

3. `astrbot/core/event_bus.py`
   - Consumes queued events and dispatches to a configured scheduler.
   - Rust target: keep EventBus thin; no provider, plugin or send logic here.

4. `astrbot/core/pipeline/scheduler.py`
   - Runs ordered stages and supports onion-style async generator stages.
   - Rust target: model as ordered async middleware/stage chain with explicit `PipelineControl`.

5. `astrbot/core/pipeline/process_stage/stage.py`
   - Chooses plugin handlers and/or Agent Provider flow.
   - Rust target: make plugin activation and provider request generation separate stages.

6. `astrbot/core/pipeline/respond/stage.py`
   - Converts `MessageEventResult` into platform send operations.
   - Rust target: keep result normalization separate from platform-specific serialization.

## Extension Systems

### Platform

Key files:

- `astrbot/core/platform/platform.py`
- `astrbot/core/platform/manager.py`
- `astrbot/core/platform/register.py`

Rust idea:

- `astrbot-platform-api`: event/session/message traits and DTOs.
- `astrbot-platform-adapters`: concrete adapters behind feature flags or separate crates.
- Manager only knows factories and runtime handles.

### Provider

Key files:

- `astrbot/core/provider/provider.py`
- `astrbot/core/provider/manager.py`
- `astrbot/core/provider/register.py`
- `astrbot/core/provider/func_tool_manager.py`

Rust idea:

- Separate `ChatProvider`, `SttProvider`, `TtsProvider`, `EmbeddingProvider`, `RerankProvider` traits.
- Provider selection is session-aware and config-driven.
- Tool calls are typed protocol data, not provider-specific JSON in core.

### Plugin / Star

Key files:

- `astrbot/core/star/star.py`
- `astrbot/core/star/star_manager.py`
- `astrbot/core/star/star_handler.py`
- `astrbot/core/star/register/star_handler.py`
- `astrbot/core/star/filter/*`

Rust idea:

- Keep Star's concepts: metadata, handlers, event types, filters, priority.
- Replace Python decorators/dynamic import with explicit registration builders.
- Plugin host should expose a narrow `PluginContext`, not all managers directly.

### Agent / Tools

Key files:

- `astrbot/core/agent/*`
- `astrbot/core/astr_agent_context.py`
- `astrbot/core/astr_agent_tool_exec.py`
- `astrbot/core/tools/*`

Rust idea:

- Agent runner depends on `ChatProvider`, `ToolExecutor`, `ConversationStore` and `RunHooks`.
- Tool execution is a capability boundary with timeout, cancellation, audit and redaction.

## Initial Crate Sketch

- `astrbot-core`: shared types, errors, config traits, event bus traits.
- `astrbot-message`: message components, chains, sessions, platform-neutral event DTOs.
- `astrbot-pipeline`: stage trait, scheduler, built-in stage order.
- `astrbot-platform`: platform trait, registry, mock/webchat adapter.
- `astrbot-provider`: provider traits, registry, OpenAI-compatible adapter.
- `astrbot-plugin`: metadata, handlers, filters, plugin context.
- `astrbot-agent`: provider request, tool loop, run context.
- `astrbot-storage`: repositories for config, conversation, persona, history.
- `astrbot-cli`: init/run/dev commands.
- `astrbot-dashboard`: later HTTP API and static dashboard serving.

## First Milestone Candidate

Build the minimal typed message loop:

1. Rust workspace and crate boundaries.
2. `MessageEvent`, `MessageChain`, `PipelineResult`, `ChatProvider` traits.
3. Mock Platform -> EventBus -> Pipeline -> Mock Provider -> Respond test.
4. CLI command to run a local mock/webchat loop.

