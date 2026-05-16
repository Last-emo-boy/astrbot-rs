# Refactor Decoupling Audit

## Intent

Pause new AstrBot parity migration and record the next structural refactor backlog in Maestro before adding more platforms, providers, or dashboard surface.

## Evidence

| Area | Current hotspot | Evidence | Decoupling target |
| --- | --- | --- | --- |
| Runtime | `crates/astrbot-runtime/src/lib.rs` | 2456 lines; `RuntimeConfig` at line 38, `AstrbotRuntime` at line 1352, manager builders at lines 1626-1689 | Keep `lib.rs` as facade; split config, provider/policy config, assembly, handle, ports, config IO |
| Provider | `crates/astrbot-provider/src/registry.rs` | 2085 lines; constants at lines 26-51, config at line 114, `ProviderRegistry` at line 821, factories at line 1241+, `ProviderManager` at line 1801 | Split constants/capability/config/factories/options/registry/manager |
| Provider tests | `crates/astrbot-provider/tests/provider_registry.rs` | 1386 lines | Split tests by chat, STT, TTS, embedding, rerank, lifecycle |
| WebChat | `crates/astrbot-web/src/lib.rs` | 810 lines; DTOs at lines 21-33, router at line 133, conversion at lines 181 and 265, tests at line 329 | Split DTO, message parts, routes, server, errors, tests |
| Core message | `crates/astrbot-core/src/message.rs` | 841 lines; components at line 9, chain at line 129, session at line 277, sink at line 338, provider request at line 473, event at line 674 | Split message domain into focused submodules while preserving `astrbot_core::message::*` |
| Pipeline context | `crates/astrbot-pipeline/src/context.rs` | 584 lines; context at line 14, policy config/ports at lines 181-540, result config at line 668 | Split policy configs, ports, session state, content safety, provider preference, result config |
| Provider root | `crates/astrbot-provider/src/lib.rs` | 512 lines | Move public traits/request/response/mock providers into capability modules; keep root as re-export facade |

## AstrBot Comparison

AstrBot's provider manager keeps capability buckets separated: chat providers, STT providers, TTS providers, embedding providers, and rerank providers are loaded and terminated independently in `E:/Playground/Astrbot/astrbot/core/provider/manager.py`. The Rust version already mirrors this idea at the trait/manager level, but `registry.rs` now combines the buckets, factories, configs, and manager implementation in one file. The next split should preserve AstrBot's conceptual buckets while using Rust modules and re-exports to keep compile-time boundaries clear.

AstrBot's platform registration also separates adapter registration from platform manager loading through `E:/Playground/Astrbot/astrbot/core/platform/register.py` and `manager.py`. The recent Rust platform split follows that direction; the same facade-root pattern should now be applied to runtime, provider, web, core message, and pipeline context.

## Priority

1. Runtime split, because every new config/provider/platform feature currently expands `lib.rs`.
2. Provider registry/manager split, because provider parity work keeps adding constants, config, factories, manager methods, and tests into the same files.
3. Provider trait/test split, because capability-specific tests should stop sharing one large registry integration file.
4. Web split, before dashboard/API breadth grows.
5. Core message split, before more platform media segments and provider request DTOs land.
6. Pipeline context split, before more policy/session/memory ports accumulate.

## Decision

Before continuing provider or platform parity, run `M7-R3-runtime-provider-decoupling` and `M7-R4-web-core-pipeline-decoupling`. The first implementation pass should move code without behavior changes, preserve public re-exports, and verify package tests after each split.

## Execution Update

`TASK-001` is completed. `crates/astrbot-runtime/src/lib.rs` is now a facade, and runtime implementation is split across:

- `config.rs`
- `provider_config.rs`
- `policy_config.rs`
- `platform_config.rs`
- `handle.rs`
- `assembly.rs`
- `ports.rs`
- `defaults.rs`
- `config_io.rs`
- `tests.rs`

Verification passed: `cargo check -p astrbot-runtime`, `cargo fmt --all --check`, `cargo test -p astrbot-runtime`, and `cargo test --workspace`.

`TASK-002` is also completed. `crates/astrbot-provider/src/registry.rs` is now focused on registry behavior, while provider constants, capability metadata, config structs, concrete factories, and manager routing live in separate modules:

- `constants.rs`
- `capability.rs`
- `config.rs`
- `factories.rs`
- `manager.rs`
- `registry.rs`

Verification passed: `cargo check -p astrbot-provider`, `cargo test -p astrbot-provider`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings`.

`TASK-003` is completed. `crates/astrbot-provider/src/lib.rs` is now a public facade, while provider traits/request/response/mock implementations are grouped by capability:

- `chat.rs`
- `speech.rs`
- `tts.rs`
- `embedding.rs`
- `rerank.rs`
- `mock.rs`

`crates/astrbot-provider/tests/provider_registry.rs` now contains shared helpers and imports capability-specific test modules from:

- `tests/provider_registry/chat.rs`
- `tests/provider_registry/speech.rs`
- `tests/provider_registry/tts.rs`
- `tests/provider_registry/embedding.rs`
- `tests/provider_registry/rerank.rs`
- `tests/provider_registry/lifecycle.rs`

Verification passed: `cargo fmt --all --check`, `cargo test -p astrbot-provider`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings`.

## Follow-Up Decoupling Audit

After the first three tasks, the remaining first-pass blockers are still `astrbot-web`, `astrbot-core::message`, and `astrbot-pipeline::context`. These should happen before new dashboard, platform media, or provider breadth:

- WebChat: `crates/astrbot-web/src/lib.rs` still mixes DTOs, message-part conversion, routes, server helpers, error mapping, and tests. This maps to AstrBot's `platform/sources/webchat/message_parts_helper.py` boundary and should become `dto`, `message_parts`, `routes`, `server`, `error`, and tests modules.
- Core message: `crates/astrbot-core/src/message.rs` still combines message components, chain utilities, session/sender, sinks/streams/results, provider request DTOs, tool placeholders, and event state. This should follow AstrBot's split between `core/message/components.py`, `platform/astrbot_message.py`, `platform/astr_message_event.py`, and `message/message_event_result.py`.
- Pipeline context: `crates/astrbot-pipeline/src/context.rs` still combines context assembly, policy config, policy ports, in-memory provider preference state, quote context policy, content safety strategies, fallback config, and result decoration config. AstrBot keeps pipeline context thin in `pipeline/context.py` while stage-specific policies live under stage folders; Rust should split this into typed context submodules.

Additional second-pass items are now in `plan.json` as `TASK-007` through `TASK-012`:

- Split runtime provider config mappings and runtime tests by capability/lifecycle.
- Split provider config and factory modules by capability/family.
- Split plugin crate into SDK, handler registry, filters, manifest, and sandbox capability modules.
- Prepare platform adapter transport/message-conversion submodules before WeChat/QQ parity.
- Split large integration test files by behavior area.
- Extract WebChat attachment/message-part service boundaries before dashboard media storage.

Next concrete task: `M7-R4-web-core-pipeline-decoupling`, starting with `TASK-004` (`crates/astrbot-web/src/lib.rs`).

## TASK-004 Execution Update

`TASK-004` is completed. `crates/astrbot-web/src/lib.rs` is now a facade with stable re-exports. The HTTP boundary is split into:

- `dto.rs`
- `message_parts.rs`
- `error.rs`
- `routes.rs`
- `server.rs`
- `tests.rs`

This keeps the route handlers thin: they receive typed DTOs, call `WebChatPlatform`, and use `message_parts.rs` for `MessageChain` normalization/readback. The split follows AstrBot's `platform/sources/webchat/message_parts_helper.py` idea, but keeps the Rust boundary typed and module-local.

Verification passed: `cargo test -p astrbot-web`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings`.

Next concrete task: `TASK-005`, split `crates/astrbot-core/src/message.rs` into focused message domain modules while preserving public imports.

## TASK-005 Execution Update

`TASK-005` is completed. `crates/astrbot-core/src/message.rs` has been replaced by a `message/` module tree with facade re-exports in `message/mod.rs`:

- `component.rs`
- `chain.rs`
- `session.rs`
- `sink.rs`
- `result.rs`
- `provider_request.rs`
- `event.rs`
- `tests.rs`

The split keeps the current `astrbot_core::message::*` and crate-root exports stable. This follows AstrBot's separation of message components, platform message/session metadata, and event result concepts while using Rust typed DTOs for provider requests and tool placeholders.

Verification passed: `cargo test -p astrbot-core`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings`.

Next concrete task: `TASK-006`, split `crates/astrbot-pipeline/src/context.rs`.

## TASK-006 Execution Update

`TASK-006` is completed. `crates/astrbot-pipeline/src/context.rs` has been replaced by a focused `context/` module tree:

- `context/mod.rs`
- `context/policy.rs`
- `context/session.rs`
- `context/provider_preference.rs`
- `context/quote.rs`
- `context/content_safety.rs`
- `context/result.rs`

The split keeps `PipelineContext` as the composition facade while moving policy configs, injected ports, provider preference storage, quote enrichment, content safety, and result decoration into their own modules. This mirrors AstrBot's thinner `pipeline/context.py` and stage-specific policy directories, but keeps Rust's typed port and config boundaries.

Verification passed: `cargo fmt --all --check`, `cargo test -p astrbot-pipeline`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings`.

## Post TASK-006 Decoupling Audit

After completing the first-pass runtime/provider/web/core/pipeline splits, the remaining decoupling space is now mostly second-pass and growth-boundary work:

- Runtime second pass: `provider_config.rs` and `tests.rs` remain large because capability-specific provider mapping and lifecycle/config/platform tests still share broad modules.
- Provider second pass: `config.rs`, `factories.rs`, and `manager.rs` still aggregate all capability buckets; this should split by chat, STT, TTS, embedding, and rerank before more provider families are added.
- Plugin SDK/sandbox: `astrbot-plugin/src/lib.rs` is still small, but it already contains event types, filters, handler metadata, handler trait, registry, and lifecycle termination. Before plugin loading grows, split it into SDK, handler, registry, filter, manifest, sandbox, and tests modules.
- Platform adapters: current platform files are manageable, but WeChat/QQ/OneBot parity needs adapter-specific `event`, `message`, and `transport` modules, matching AstrBot's `platform/sources/*/*_event.py` and adapter split.
- Pipeline registry: `registry.rs` now combines stage constants/order, built-in stage registration, registry entries, scheduler construction, and registry tests. This maps to AstrBot's separate `stage_order.py`, `bootstrap.py`, and `scheduler.py` concepts.
- Provider HTTP helpers: concrete provider modules repeat API-base normalization, bearer/header assembly, JSON error extraction, and request helper patterns. Shared helpers should be introduced conservatively so provider-specific payload mapping stays local.
- CLI entrypoint: `main.rs` now combines command parsing, command handlers, WebChat server launch, and smoke behavior. Split before dashboard/server flags increase.

New tasks recorded in `plan.json` and `.task/`:

- `TASK-013`: split pipeline stage registry order, builtins, entries, and registry tests.
- `TASK-014`: extract provider HTTP base URL, error parsing, and request helper boundaries.
- `TASK-015`: split CLI command parsing, command handlers, and WebChat server launcher.

Next concrete task remains `M7-R5-plugin-sdk-sandbox-design` / `TASK-009`, because plugin and sandbox boundaries affect all later AstrBot Star parity work.

## TASK-009 Execution Update

`TASK-009` is completed. `crates/astrbot-plugin/src/lib.rs` is now a facade that re-exports a Rust-native plugin SDK surface while keeping existing imports stable.

Implementation modules:

- `event.rs`: plugin event/control enums, expanded to cover AstrBot Star lifecycle/tool events.
- `handler.rs`: handler metadata, handler trait, and registered handler wrapper.
- `registry.rs`: priority-ordered handler registry and termination flow.
- `filter/command.rs`: command/alias/prefix filter.
- `filter/regex.rs`: regex message filter.
- `filter/platform.rs`: platform ID/name filter.
- `filter/permission.rs`: typed member/admin/owner permission filter.
- `filter/event_type.rs`: direct/group message-session-kind filter.
- `manifest.rs`: plugin manifest and capability declarations.
- `sandbox.rs`: plugin permissions, tool capabilities, sandbox profiles, and `SandboxRuntime` resolver trait.
- `sdk.rs`: `PluginContext`, `PluginModule`, SDK version, and `PluginTestHarness`.
- `tests.rs`: registry, filter, manifest, and sandbox coverage.

AstrBot comparison:

- Preserves Star handler metadata, event type grouping, priority ordering, and filter composition from `star/star_handler.py` and `star/filter/*`.
- Turns AstrBot's broad `Context` plugin surface into a typed `PluginContext` plus manifest-declared permissions.
- Moves tool execution capability checks from an ad-hoc runtime check toward reusable Rust types: `PluginPermission`, `ToolCapability`, `SandboxProfile`, and `SandboxRuntime`.

Verification passed: `cargo fmt --all --check`, `cargo test -p astrbot-plugin`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings`.

Next concrete task: `M7-R6-runtime-provider-second-pass-decoupling`, starting with `TASK-007` and `TASK-008` depending on whether runtime config/test split or provider config/factory/manager split is the safer first cut.

## TASK-007 Execution Update

`TASK-007` is completed. Runtime provider config mapping now follows AstrBot's provider capability bucket idea while staying Rust-typed and facade-compatible:

- `provider_config.rs`
- `provider_config/chat.rs`
- `provider_config/speech.rs`
- `provider_config/tts.rs`
- `provider_config/embedding.rs`
- `provider_config/rerank.rs`

Runtime tests are also split by behavior instead of sharing one large module:

- `tests.rs`
- `tests/message_loop.rs`
- `tests/provider_config.rs`
- `tests/policy.rs`
- `tests/platform.rs`
- `tests/lifecycle.rs`
- `tests/config_io.rs`

Verification passed: `cargo fmt --all --check`, `cargo test -p astrbot-runtime`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings`.

Next concrete task inside `M7-R6`: `TASK-008`, split provider config/factories/manager by capability while preserving `ProviderRegistry` and `ProviderManager` public behavior.

## Additional Decoupling Audit - Post TASK-007/TASK-009

The next scan compared current Rust hotspots with AstrBot's larger manager boundaries:

- AstrBot `platform/manager.py` keeps task wrapping, status/error recording, reload, and termination separate from adapter registration. Rust `RuntimeHandle` and `PlatformManager` should gain a task-supervision boundary before real WeChat/QQ/OneBot transport grows.
- AstrBot `provider/manager.py` confirms the current `TASK-008` direction: capability buckets are correct, but config/factory/manager logic should be split by chat, STT, TTS, embedding, and rerank.
- AstrBot `star/star_manager.py` is a warning case: discovery, metadata, dependencies, import/reload, lifecycle, uninstall, tool cleanup, and platform-extension cleanup all share one large manager. Rust should keep `astrbot-plugin` loader/lifecycle/dependency/hot-reload boundaries separate before implementing Star parity.
- AstrBot `astr_agent_tool_exec.py` mixes local tool execution, MCP, handoff, background tasks, wake-up, and sandbox capability checks. Rust should introduce a typed tool execution boundary that consumes `SandboxProfile`/`ToolCapability` instead of growing pipeline stages directly.
- AstrBot `config/default.py` is very broad and feeds dashboard metadata. Rust should split config defaults, schema/version policy, env/secret resolution, and UI metadata before dashboard config editing expands.

New tasks recorded in `plan.json` and `.task/`:

- `TASK-016`: provider test HTTP fixtures and request capture helper extraction.
- `TASK-017`: runtime handle task supervision and restart state helper split.
- `TASK-018`: plugin loader, lifecycle, dependency, and hot-reload boundary definition.
- `TASK-019`: tool execution, handoff, background task, and sandbox capability boundary.
- `TASK-020`: config defaults, schema metadata, env/secret resolution, and migration policy split.

No Rust source was changed in this audit pass. The immediate execution order remains `TASK-008` first, then platform/WebChat and pipeline/provider HTTP boundaries, because those are closer to current large files and lower risk than dynamic plugin/tool execution work.

## TASK-008 Execution Update

`TASK-008` is completed. Provider-side config, factory, and manager boundaries now follow the same capability-bucket shape as AstrBot's `ProviderManager`, but keep Rust's typed facade exports stable.

Implementation modules:

- `config.rs` facade plus `config/chat.rs`, `config/speech.rs`, `config/tts.rs`, `config/embedding.rs`, and `config/rerank.rs`.
- `factories.rs` facade plus `factories/chat.rs`, `factories/speech.rs`, `factories/tts.rs`, `factories/embedding.rs`, `factories/rerank.rs`, and shared `factories/common.rs`.
- `manager.rs` facade plus `manager/config_set.rs`, `manager/lifecycle.rs`, and per-capability routing modules under `manager/`.

AstrBot comparison:

- Mirrors AstrBot's separate provider buckets for chat, STT, TTS, embedding, and rerank.
- Keeps concrete provider payload construction in provider-specific factory/provider code instead of moving it into the registry.
- Preserves `ProviderRegistry` and `ProviderManager` as the public entry points for runtime assembly.

Verification passed: `cargo fmt --all --check`, `cargo test -p astrbot-provider`, `cargo clippy -p astrbot-provider -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings`.

Next concrete task: `M7-R7-platform-webchat-media-boundaries` / `TASK-010` and `TASK-012`, preparing platform adapter transport/message conversion and WebChat attachment/message-part services before WeChat/QQ and dashboard media work.

## TASK-010 / TASK-012 Execution Update

`TASK-010` and `TASK-012` are completed as the `M7-R7` platform/WebChat boundary pass.

Platform adapter modules:

- `adapters/onebot/mod.rs`
- `adapters/onebot/event.rs`
- `adapters/onebot/message.rs`
- `adapters/webchat/mod.rs`
- `adapters/webchat/event.rs`
- `adapters/webchat/message.rs`

WebChat HTTP/service modules:

- `attachment.rs`: attachment resolution port and passthrough service for future upload/storage wiring.
- `history.rs`: conversation history DTO assembly boundary.
- `routes.rs`: list-message handler now delegates history response assembly instead of building DTOs inline.

AstrBot comparison:

- Mirrors AstrBot's adapter-local event/message conversion pattern under `platform/sources/*`.
- Keeps `PlatformRegistry` and `PlatformManager` as the runtime entry points; adapter-specific conversion stays under adapters.
- Preserves AstrBot WebChat helper direction while using Rust typed DTOs and service ports.

Verification passed: `cargo fmt --all --check`, `cargo test -p astrbot-platform`, `cargo test -p astrbot-web`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings`.

Next concrete task: `M7-R8-pipeline-registry-boundary` / `TASK-013`, splitting pipeline stage order constants, built-in registration, registry entries, and registry tests.

## Additional Decoupling Audit - Post TASK-010/TASK-012

After the platform/WebChat split, I scanned current Rust hotspots and compared them with AstrBot's larger boundaries under `platform/sources/*`, `provider/sources/*`, `astr_main_agent.py`, `agent/runners/tool_loop_agent_runner.py`, `db/sqlite.py`, and dashboard-facing management surfaces.

New decoupling space recorded:

- WebChat tests: `crates/astrbot-web/src/tests.rs` is now the largest Rust test module and mixes submit, message-part validation, history, and live server behavior. Record `TASK-021`.
- Provider protocol DTO/parsers: concrete adapters like `openai_compatible.rs`, `gemini.rs`, `minimax_tts.rs`, and Xinference adapters still combine protocol DTOs, payload builders, response parsing, SSE parsing, and adapter orchestration. `TASK-014` covers shared HTTP helpers; record `TASK-022` for protocol-local DTO/parser extraction.
- Platform transport/session/media: `TASK-010` split adapter event/message conversion, but real OneBot/QQ/WeChat parity needs transport lifecycle, reconnect, session metadata, and media upload/download boundaries. Record `TASK-023`.
- Storage ports: WebChat history, attachment metadata, provider preferences, and config snapshots should not bind directly to platform/runtime modules once persistence arrives. AstrBot's `db`, `backup`, and WebChat history migrations are the warning reference. Record `TASK-024`.
- Agent runner boundary: Rust pipeline still calls provider flows directly from stages, while AstrBot has a main-agent layer for request decoration, persona, context, fallback, and tool loop orchestration. Record `TASK-025` before these concerns grow into `ProcessStage`.
- Management API boundary: WebChat chat transport should stay separate from dashboard provider/platform/plugin/config/status APIs. Record `TASK-026`.

These tasks are intentionally pending backlog items. They do not change the immediate execution pointer: the next concrete implementation task remains `M7-R8-pipeline-registry-boundary` / `TASK-013`, unless we explicitly prioritize `TASK-011` test splitting first.

## TASK-013 Execution Update

`TASK-013` is completed. `crates/astrbot-pipeline/src/registry.rs` is now a focused facade/core registry module, and the stage registry boundary is split into:

- `registry/order.rs`: stage type and order constants.
- `registry/builtins.rs`: deterministic built-in stage registration.
- `registry/entry.rs`: registered stage entry and factory wrapper.
- `registry/tests/registration.rs`: duplicate and invalid type registration coverage.
- `registry/tests/order.rs`: order and tie-break behavior coverage.
- `registry/tests/builtins.rs`: built-in pipeline order and scheduler construction coverage.
- `registry/tests/scheduler.rs`: initialization and stop-control behavior coverage.

AstrBot comparison:

- Mirrors `stage_order.py` by isolating stage ordering constants.
- Mirrors `bootstrap.py` by isolating built-in registration from registry storage.
- Preserves Rust's `DefaultPipelineBuilder` / `PipelineScheduler` typed construction instead of relying on import side effects.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-pipeline`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R9-provider-http-helper-boundary` / `TASK-014`.

## TASK-014 Execution Update

`TASK-014` is completed. Provider adapters now share a small HTTP helper boundary:

- `http/auth.rs`: bearer/API-key header helpers and custom header insertion.
- `http/client.rs`: reqwest client construction with shared timeout/header behavior.
- `http/error.rs`: structured JSON error extraction for common provider error shapes.
- `http/url.rs`: API base/path joining without adapter-local slash handling.

Updated adapters include OpenAI-compatible chat, OpenAI embedding, OpenAI STT/TTS, VLLM/Xinference rerank, Xinference STT, Anthropic, and Gemini. Provider-specific payload mapping and response parsing stayed in each adapter.

AstrBot comparison:

- Follows the repeated helper patterns visible across `E:/Playground/Astrbot/astrbot/core/provider/sources/*`.
- Avoids copying AstrBot's source-level sprawl by keeping Rust shared behavior in a typed helper module and leaving adapter semantics local.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R10-cli-entrypoint-boundary` / `TASK-015`.

## Additional Decoupling Audit - Post TASK-014

After closing the provider HTTP helper boundary, I rescanned current Rust hotspots and the AstrBot reference tree. The remaining current large Rust files are mostly already covered by pending tasks: WebChat tests (`TASK-021`), pipeline process-stage tests (`TASK-011`), provider protocol DTO/parser extraction (`TASK-022`), platform tests (`TASK-011`), CLI entrypoint (`TASK-015`), and runtime handle (`TASK-017`).

New decoupling space recorded:

- Provider registry built-ins: `crates/astrbot-provider/src/registry.rs` is still a 413-line registry core that mixes built-in registration, factory type aliases, metadata lookup, duplicate checks, and per-capability build errors. Record `TASK-027`.
- Agent context window: AstrBot's `agent/context/{manager,compressor,truncator,token_counter}.py` is a clear boundary that should not be merged into `ProcessStage` or concrete providers. Record `TASK-028`.
- MCP: AstrBot splits MCP client lifecycle, resources, prompts, sampling, roots, and elicitation, while `func_tool_manager.py` shows the risk of coupling MCP to provider tool catalogs. Record `TASK-029`.
- Tool schema and command catalog: AstrBot's `func_tool_manager.py` and `star/command_management.py` mix tool schema serialization, activation, command toggles, renames, permissions, and conflicts. Rust should separate these before tool-call parity. Record `TASK-030`.
- KB/RAG: AstrBot knowledge-base code separates documents, parsers, chunking, retrieval, rank fusion, embedding, rerank, and vector stores. Record `TASK-031`.
- Persistence migration/backup: `TASK-024` covers storage ports, but AstrBot DB and backup code also need schema, migrations, stats, import, and export boundaries. Record `TASK-032`.
- Platform webhook security: WeChat/QQ/WeCom parity needs signature verification, encrypted callback payloads, callback servers, long connections, and queues apart from adapter event/message conversion. Record `TASK-033`.

These tasks are pending backlog items. They should not preempt the immediate low-risk structural sequence unless the next feature requires that boundary.

## TASK-015 Execution Update

`TASK-015` is completed. `crates/astrbot-cli/src/main.rs` is now a thin async entrypoint:

- `args.rs`: CLI command enum, parsing, and default config path.
- `commands/mod.rs`: command dispatch.
- `commands/init.rs`: init behavior.
- `commands/run.rs`: runtime start/stop behavior.
- `commands/smoke.rs`: smoke command behavior.
- `webchat_server.rs`: WebChat listener preparation, startup handle, address, and shutdown.
- `tests.rs`: parsing and WebChat server integration coverage.

AstrBot comparison:

- Learns from AstrBot's lifecycle/dashboard startup surface, but keeps Rust CLI as a narrow lifecycle adapter rather than a broad orchestration module.
- Preserves the existing rule that WebChat HTTP launches from runtime-created `WebChatPlatform` handles and does not reach into EventBus/Pipeline internals.

Verification passed:

- `cargo test -p astrbot-cli`
- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R11-provider-test-support-boundary` / `TASK-016`.

## Additional Decoupling Audit - Post TASK-016 Partial

I rechecked current Rust hotspots and the AstrBot reference tree before marking `TASK-016` complete. At that point, the shared provider test support modules existed, but several adapter tests still defined local `serve_once`, `serve_sequence`, `read_http_request`, and temp media path helpers. `TASK-016` was kept `in_progress` until the HTTP fixture migration could be finished.

New decoupling space recorded:

- Provider test media fixtures: remaining temp audio/output path helpers should move to shared support after HTTP fixtures are complete. Record `TASK-034`.
- Provider generated media artifacts: TTS adapters repeat output directory defaults, file naming, directory creation, write errors, and display path conversion. Record `TASK-035`.
- Streaming strategy: OpenAI-compatible chat and MiniMax TTS parse stream events inside concrete adapters, while AstrBot also has unsupported streaming strategy policy. Record `TASK-036`.
- Multimodal request preparation: AstrBot `astr_main_agent.py` separates image captioning, quoted image fallback, modality checks, and unsupported image replacement; Rust should keep that out of `ProcessStage` and provider serializers. Record `TASK-037`.
- Path/temp artifact policy: AstrBot centralizes data/plugin/temp paths and cleanup; Rust currently scatters temp path choices across runtime, provider media, WebChat, and tests. Record `TASK-038`.
- T2I rendering: AstrBot T2I rendering has template and strategy complexity that should not enter RespondStage or WebChat routes directly. Record `TASK-039`.
- Observability: runtime status, trace, and dashboard log streaming need typed ports before management UI work grows. Record `TASK-040`.

Immediate execution pointer remains `M7-R11-provider-test-support-boundary` / `TASK-016`; the newly recorded tasks are growth guards after the existing structural backlog.

## TASK-016 Execution Update

`TASK-016` is completed. Provider integration tests now share the HTTP fixture boundary:

- `tests/support/http_server.rs`: `serve_once`, `serve_sequence`, `TestResponse`, content-length-aware request capture, and response writing.
- `tests/support/captured_request.rs`: shared captured request header assertions.
- Migrated adapter tests: Anthropic, Gemini, OpenAI-compatible chat, OpenAI/Gemini embedding, VLLM/Bailian/Xinference rerank, OpenAI/Xinference STT, OpenAI/Gemini/Volcengine/MiniMax/GSVI TTS, and provider registry coverage.

AstrBot comparison:

- Follows the repeated provider source/test shape in `E:/Playground/Astrbot/astrbot/core/provider/sources/*`, but keeps Rust tests from copying socket-level fixtures per adapter.
- Provider-specific protocol payload and response assertions remain in each adapter test.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R12-runtime-task-supervision-boundary` / `TASK-017`. `TASK-034` remains pending but is now narrowed to provider test media/temp path helpers, since the HTTP fixture migration is complete.

## Additional Decoupling Audit - Post TASK-016 Completion

After completing provider test HTTP support, I rechecked current Rust hotspots and compared them with AstrBot boundaries that have not yet been represented in the Maestro backlog:

- Event bus routing: AstrBot `event_bus.py` routes messages to config-specific pipeline schedulers and owns event outline logging. Rust should split dispatch, scheduler routing, logging, and backpressure before multi-config pipeline dispatch grows. Record `TASK-041`.
- Provider selection state: AstrBot `provider/manager.py` owns selected/default providers and change callbacks. Rust should define selection state and change notification boundaries before dashboard/provider management APIs mutate manager internals. Record `TASK-042`.
- Conversation/history domain: AstrBot separates `conversation_mgr.py`, `platform_message_history_mgr.py`, and history persistence helpers from raw DB code. Rust should define conversation/history domain services beyond low-level storage ports. Record `TASK-043`.
- Persona/skills prompt composition: AstrBot `persona_mgr.py` and `skills/skill_manager.py` feed main-agent prompt construction. Rust should keep persona resolution, skill activation, and prompt rendering outside `ProcessStage` and plugin SDK. Record `TASK-044`.
- Cron/proactive jobs: AstrBot `cron/manager.py` stores scheduled jobs, wakes the main agent with special events, and persists history. Rust should isolate scheduled/proactive flows from runtime handle and WebChat transport. Record `TASK-045`.
- Subagent handoff orchestration: AstrBot `subagent_orchestrator.py` registers handoff tools without executing agents itself. Rust should keep subagent config, handoff registration, provider override, and tool bridge logic separate from generic tool execution and sandbox policy. Record `TASK-046`.

These are pending growth guards. The immediate implementation pointer remains `M7-R12-runtime-task-supervision-boundary` / `TASK-017`.

## TASK-017 Execution Update

`TASK-017` is completed. `crates/astrbot-runtime/src/handle.rs` has been replaced by a focused `handle/` module tree:

- `handle/mod.rs`: public facade re-export.
- `handle/runtime.rs`: `AstrbotRuntime`, `RuntimeHandle`, runtime initialization, accessors, start/stop, and public mock/sent-message helper methods.
- `handle/supervisor.rs`: event-bus/platform background task spawning and abort/join error mapping.
- `handle/restart.rs`: restart state capture and provider preference restore policy.
- `handle/testing.rs`: shared mock platform event emission and sent-message readback helpers.

AstrBot comparison:

- Follows `core_lifecycle.py` and `platform/manager.py` by keeping task supervision and cancellation/error mapping separate from restart/reload state transfer.
- Preserves Rust's typed manager termination order: stop supervised tasks, then terminate plugin, provider, and platform managers.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-runtime`
- `cargo clippy -p astrbot-runtime -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R13-plugin-loader-lifecycle-boundary` / `TASK-018`.

## TASK-018 Execution Update

`TASK-018` is completed. `astrbot-plugin` now has typed loader/lifecycle boundaries without implementing dynamic plugin import:

- `loader/mod.rs`: `PluginLoader` facade over a `PluginStateStore`.
- `loader/metadata.rs`: `PluginLoadSource`, source kind, plugin ID normalization, metadata, supported platforms, runtime version marker.
- `loader/dependency.rs`: dependency descriptors, dependency plans, and `PluginDependencyInstaller` trait with a no-op implementation.
- `loader/lifecycle.rs`: lifecycle state/action/event types.
- `loader/store.rs`: `PluginRecord`, `PluginStateStore`, and `InMemoryPluginStore`.
- `loader/hot_reload.rs`: file change descriptors and reload/unload/ignore decisions.
- `extension/platform.rs`: typed plugin platform extension descriptors.
- `extension/web_api.rs`: typed plugin web API route descriptors.

AstrBot comparison:

- Splits `StarManager` concerns for metadata, dependency install, lifecycle state, hot reload, platform cleanup, and web API registration into Rust-native typed surfaces.
- Keeps native Rust plugins and future Python compatibility bridges possible without coupling loader code to sandbox execution or the handler registry.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-plugin`
- `cargo clippy -p astrbot-plugin -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R14-tool-execution-sandbox-boundary` / `TASK-019`.

## TASK-019 Execution Update

`TASK-019` is completed. `astrbot-plugin` now has a typed tool execution and sandbox-capability boundary:

- `tool/mod.rs`: public facade for plugin tool APIs.
- `tool/declaration.rs`: `PluginToolDeclaration` and `PluginToolKind` for local, MCP, handoff, and background tools.
- `tool/handoff.rs`: `HandoffToolTarget` for agent handoff descriptors.
- `tool/background.rs`: `BackgroundTaskPolicy` for long-running/background tool semantics.
- `tool/capability.rs`: `ToolCapabilityDecision` for reusable sandbox permission/capability evaluation.
- `tool/executor.rs`: `ToolExecutionRequest`, `ToolExecutionResult`, `ToolExecutionStatus`, `ToolExecutor`, and `SandboxedToolExecutor`.

AstrBot comparison:

- `E:/Playground/Astrbot/astrbot/core/astr_agent_tool_exec.py` mixes local tool calls, MCP tools, handoff agents, background tasks, wake-up behavior, and sandbox checks. Rust now keeps those concepts typed and separate before real execution is wired.
- Plugin SDK and loader code can declare tools without owning concrete tool execution, which leaves room for MCP, subagent, and computer-use execution to attach later.

Verification passed before this Maestro sync:

- `cargo fmt --all --check`
- `cargo test -p astrbot-plugin`
- `cargo test -p astrbot-pipeline`
- `cargo clippy -p astrbot-plugin -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R15-config-schema-boundary` / `TASK-020`.

## Additional Decoupling Audit - Post TASK-019

After completing the tool execution boundary, I rescanned current Rust hotspots and compared them with AstrBot areas not yet represented clearly in the Maestro backlog.

New decoupling space recorded:

- Computer-use runtime: AstrBot `computer/computer_client.py` and `computer_tool_provider.py` combine sandbox booters, shell/python/filesystem/browser components, skill sync, prompt fragments, and tool exposure. Record `TASK-047`.
- Plugin dependency environment: `utils/pip_installer.py` shows dependency installation, isolated site-packages preference, conflict classification, core constraints, frozen-runtime patches, and secret redaction. Record `TASK-048`.
- Platform API clients: rich adapters such as Misskey/Lark/DingTalk/Telegram carry API clients, retry/rate-limit behavior, WebSocket errors, and rich event DTO normalization beyond transport/session/media. Record `TASK-049`.
- Quote/forward parsing: `utils/quoted_message/chain_parser.py` handles reply chains, OneBot forward nodes, nested text, and image references outside the main pipeline. Record `TASK-050`.
- Live agent feedback: `astr_agent_run_util.py` coordinates streaming chunks, tool status, stop signals, final chain extraction, and optional TTS/live feedback. Record `TASK-051`.

These are growth guards, not the next implementation target. The immediate pointer remains `M7-R15-config-schema-boundary` / `TASK-020`.

## TASK-020 Execution Update

`TASK-020` is completed. Runtime config now has typed boundaries for defaults, env loading, secret redaction, migration/default merge planning, schema metadata, and dashboard-facing UI metadata:

- `config/defaults.rs`: default constants/functions moved under the config boundary; root `defaults.rs` remains a compatibility facade for existing modules.
- `config/env.rs`: `RuntimeEnvConfigSource` and `runtime_config_from_env()` separate env lookup from `RuntimeConfig`.
- `config/secrets.rs`: `SecretValue`, `REDACTED_SECRET`, and redaction helpers prevent debug/display leakage of secret values.
- `config/migration.rs`: `RuntimeConfigMigrationPlan` reports missing top-level and nested defaults before config IO writes normalized files.
- `config/schema.rs`: `RuntimeConfigSchema` and `ConfigFieldSchema` expose typed fields, defaults, value types, and secret markers.
- `config/ui_metadata.rs`: `ConfigUiMetadata` groups fields into dashboard-ready control metadata.
- `tests/config_schema.rs`: covers env-derived provider config, default env fallbacks, secret redaction, migration plans, schema fields, and UI control mapping.

AstrBot comparison:

- AstrBot's `config/default.py` combines defaults and dashboard metadata, while `config/astrbot_config.py` performs schema/default integrity work. Rust now keeps these concerns typed and separately testable before dashboard config editing grows.
- Env/secret handling is now testable through lookup injection instead of mutating process environment in tests.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-runtime`
- `cargo clippy -p astrbot-runtime -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R16-web-test-boundary` / `TASK-021`.

## TASK-021 Execution Update

`TASK-021` is completed. WebChat HTTP tests are now split by behavior area:

- `tests.rs`: facade only.
- `tests/support.rs`: shared WebChat fixture, router helpers, JSON response parsing, and runtime sent-message waiting.
- `tests/submit.rs`: text, image-only, and message-parts submit route coverage.
- `tests/message_parts.rs`: reply preservation, reply-only rejection, non-image media, and empty payload behavior.
- `tests/history.rs`: recorded WebChat history serialization/readback.
- `tests/server.rs`: bound TCP server smoke tests and runtime reply history readback.

AstrBot comparison:

- This follows the same boundary idea as AstrBot WebChat's message-part helper while keeping route-unit and live-server tests separate.
- No WebChat HTTP behavior changed; this is a test-structure split only.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-web`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R17-provider-protocol-boundary` / `TASK-022`.

## TASK-022 Execution Update

`TASK-022` is completed. Provider protocol DTOs and parsers are now split out of the concrete adapter orchestration files:

- `protocol/sse.rs`: shared SSE `data:` line extraction.
- `protocol/openai_chat.rs`: OpenAI-compatible chat payload construction, non-stream response parsing, and streaming content collection.
- `protocol/gemini_chat.rs`: Gemini `generateContent` payloads, data URL image conversion, blocked finish-reason parsing, and focused tests.
- `protocol/anthropic_chat.rs`: Anthropic messages payloads, data URL image blocks, content extraction, and focused tests.
- `protocol/minimax_tts.rs`: MiniMax request DTOs, SSE hex audio collection, hex decode, error extraction, and focused tests.
- `protocol/xinference.rs`: Xinference list/launch model parser, STT text parser, rerank parser, and shared request DTOs.

Concrete adapters now keep config, URL/header construction, HTTP send/read, media loading or artifact writing, and trait implementation local. Protocol-specific payload and response mapping no longer sits inside `openai_compatible.rs`, `gemini.rs`, `anthropic.rs`, `minimax_tts.rs`, `xinference_stt.rs`, or `xinference_rerank.rs`.

AstrBot comparison:

- AstrBot provider sources keep wire-shape preparation close to each source file; Rust keeps the same provider-local knowledge but gives it a module boundary so adapter lifecycle code stays thin.
- MiniMax SSE audio and OpenAI-compatible streaming now share an SSE helper instead of each adapter manually splitting event blocks.
- Xinference STT/Rerank now share response parsing, while model UID resolution remains a separate follow-up boundary.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R18-platform-transport-boundary` / `TASK-023`.

## Additional Decoupling Audit - Post TASK-022

After completing the provider protocol boundary, I rescanned current large Rust files and compared them with AstrBot provider sources.

New decoupling space recorded:

- Remaining non-chat provider protocol DTO/parsers: `gemini_tts.rs`, `gemini_embedding.rs`, `volcengine_tts.rs`, `openai_embedding.rs`, `openai_tts.rs`, `openai_stt.rs`, `bailian_rerank.rs`, and `vllm_rerank.rs` still keep provider protocol DTOs and response parsing in concrete adapters. Record `TASK-052`.
- Xinference model resolver lifecycle: `xinference_stt.rs` and `xinference_rerank.rs` still duplicate model UID cache, running model lookup, optional model launch, and disabled auto-launch errors. Record `TASK-053`.

These are follow-up guardrails. The immediate implementation pointer remains `M7-R18-platform-transport-boundary` / `TASK-023`.

## TASK-023 Execution Update

`TASK-023` is completed. Platform transport/session/media boundaries now exist before real OneBot/QQ/WeChat network parity expands:

- `adapters/common/transport.rs`: defines `PlatformTransport`, `PlatformTransportKind`, `PlatformTransportState`, and `NoopTransport`.
- `adapters/common/media.rs`: defines platform media upload/reference DTOs and `PlatformMediaUploadClient`.
- `adapters/onebot/session.rs`: defines `OneBotSession` and `OneBotSessionKind` for private/group session metadata and conversation ID generation.
- `adapters/onebot/transport.rs`: defines `OneBotTransport` and `OneBotTransportMode` for in-process and reverse-WebSocket lifecycle state.
- `OneBotPlatform` now delegates `run()` and `terminate()` through the transport boundary while keeping submit/event behavior unchanged.
- `astrbot-platform` re-exports the new boundary types so future runtime/dashboard/platform work can depend on typed surfaces rather than adapter internals.

AstrBot comparison:

- AstrBot `PlatformManager` supervises adapter `run()`/`terminate()` tasks; Rust now has an explicit transport trait behind the adapter run path.
- AstrBot OneBot and QQ official adapters keep session scene/message IDs for outbound routing; Rust now has a OneBot session metadata boundary before adding richer send-by-session behavior.
- AstrBot platform adapters perform media upload in platform-specific code; Rust now has a media upload DTO/client boundary before those APIs are implemented.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-platform`
- `cargo clippy -p astrbot-platform -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R19-storage-boundary` / `TASK-024`.

## TASK-024 Execution Update

`TASK-024` is completed. A new `astrbot-storage` crate now defines repository ports and in-memory implementations for:

- conversation history
- attachment metadata
- provider preference
- config snapshots

Rust changes:

- `WebChatPlatform` exposes conversation history through `ConversationHistoryRepository`.
- `RecordingSink` implements the conversation history repository so current WebChat behavior stays in-memory but is no longer route-owned.
- WebChat history routes read storage records and map storage errors instead of silently returning empty history.
- Pipeline provider preference now delegates to `ProviderPreferenceRepository`.
- Runtime restart snapshots/restores provider preference asynchronously through the storage-backed port.

AstrBot comparison:

- Follows AstrBot's `db/sqlite.py` and backup/import/export direction by introducing low-level storage ports before adding persistent backends or dashboard history APIs.
- Keeps the current Rust runtime simple: storage backends are deferred, but upper layers no longer have to know where history/preferences will live.

Verification passed:

- `cargo fmt --all`
- `cargo test -p astrbot-storage`
- `cargo test -p astrbot-pipeline`
- `cargo test -p astrbot-platform`
- `cargo test -p astrbot-web`
- `cargo test -p astrbot-runtime`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

Next concrete task: `M7-R20-agent-runner-boundary` / `TASK-025`.

## Additional Decoupling Audit - Post TASK-024

After completing storage ports, I rescanned current Rust hotspots and compared them again with AstrBot manager/config/provider-test patterns. Four extra boundaries were not yet precisely represented by `TASK-025` through `TASK-053`, so they are now recorded:

- Provider manager bucket boundary: `ProviderManager::from_configs` still repeats map/default/accessor logic across chat, STT, TTS, embedding, and rerank buckets. Record `TASK-054`.
- Plugin test boundary: `crates/astrbot-plugin/src/tests.rs` now covers registry, filters, manifest/SDK, loader, dependency, extensions, sandbox, and tool execution in one module. Record `TASK-055`.
- Runtime policy config boundary: `policy_config.rs` still groups wake, whitelist, session, rate limit, content safety, provider fallback, result decoration, and restart state DTOs/conversions. Record `TASK-056`.
- Provider TTS factory option boundary: `factories/tts.rs` mixes provider option aliases, typed parsing, defaults, custom headers, and concrete config mapping for multiple TTS providers. Record `TASK-057`.

These are follow-up guardrails. They should not preempt `TASK-025` unless the next work touches provider manager internals, plugin Star parity tests, runtime `platform_settings` parity, or new TTS provider option schemas.

## TASK-025 Execution Update

`TASK-025` is completed. A new `astrbot-agent` crate now owns the first Rust-native agent orchestration boundary:

- `runner.rs`: `AgentRunner`, `AgentRunOutcome`, and `ChatAgentRunner`.
- `fallback.rs`: `AgentFallbackPolicy`, mirroring current provider fallback behavior without tying it to pipeline stages.
- `request_decorator.rs`: request envelope building plus provider preference, session context, quote context, and composite decorator traits.
- `persona.rs`: a typed persona prompt decorator for future persona manager integration.
- `tool_loop.rs`: tool-loop policy/state/outcome placeholders so TASK-019 tool execution can attach later without entering provider adapters or pipeline stages.

Pipeline integration:

- `ProviderStage` now constructs a `ChatAgentRunner` and delegates provider fallback through it.
- Existing `PipelineContext` ports are adapted with private wrappers that implement the agent crate's port traits.
- `ProcessStage` remains focused on plugin control flow, then calls the provider/agent boundary as before.

AstrBot comparison:

- This follows `astr_main_agent.py` and `tool_loop_agent_runner.py`: request decoration, tool loop policy, persona prompt insertion, and provider execution belong to an agent layer, not to stage control flow.
- Rust keeps the first pass smaller and typed: full tool calls, MCP, subagents, context compression, and multimodal sanitization remain future tasks, but now have a crate destination.

Verification passed:

- `cargo fmt --all`
- `cargo test -p astrbot-agent`
- `cargo test -p astrbot-pipeline`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

Next concrete task: `M7-R21-management-api-boundary` / `TASK-026`.

## TASK-026 Execution Update

`TASK-026` is completed. Dashboard-facing management APIs now live in a dedicated `astrbot-web::management` namespace instead of sharing WebChat transport route modules:

- `management/mod.rs`: management API state and router.
- `management/status.rs`: combined runtime/provider/platform/plugin status DTO.
- `management/providers.rs`: provider manager snapshot endpoint.
- `management/platforms.rs`: platform manager snapshot endpoint.
- `management/plugins.rs`: plugin registry/handler snapshot endpoint.
- `server.rs`: `serve_management` and `serve_management_with_shutdown` entry points.

Integration notes:

- WebChat submit/history routes remain transport-specific.
- Management DTOs read through manager/plugin facades and do not expose pipeline or event-bus internals.
- `PlatformManager` exposes list/count helpers for dashboard snapshots without leaking adapter storage.

AstrBot comparison:

- This follows AstrBot dashboard and manager surfaces by keeping provider, platform, and plugin status as management concerns.
- Rust keeps management reads as typed snapshots for now; mutation APIs, config editing, logs, and dashboard UI remain future tasks behind explicit boundaries.

Verification passed:

- `cargo fmt --all`
- `cargo test -p astrbot-web`
- `cargo test -p astrbot-platform`
- `cargo test -p astrbot-runtime`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

Next concrete task: `M7-R22-provider-registry-builtin-boundary` / `TASK-027`.

## Additional Decoupling Audit - Post TASK-026

After completing the management API route boundary, I rescanned current Rust large files and compared the next dashboard/runtime/plugin/session surfaces with AstrBot.

Existing coverage check:

- Current large Rust files are already mostly represented by pending tasks: `process_stage.rs` tests by `TASK-011`, provider registry/manager/TTS/protocol work by `TASK-027`, `TASK-052`, `TASK-054`, and `TASK-057`, plugin tests by `TASK-055`, runtime policy config by `TASK-056`.
- The new candidates below are not duplicates of `TASK-027` through `TASK-057`; they cover dashboard auth/mutation/realtime, plugin marketplace/update, and session concurrency.

New decoupling space recorded:

- Dashboard/OpenAPI auth and API keys: AstrBot separates JWT login, account editing, API-key hashing, scopes, and middleware in `dashboard/server.py`, `routes/auth.py`, `routes/api_key.py`, and `routes/open_api.py`. Record `TASK-058`.
- Dashboard config mutation and UMOP routing: AstrBot `routes/config.py` and `core/umop_config_router.py` show config validation, multi-config routing, provider/platform CRUD, file config, and reload policy should not live in route handlers. Record `TASK-059`.
- Plugin marketplace/update source: AstrBot `routes/plugin.py`, `routes/update.py`, `core/star/updator.py`, and `star_manager.py` separate market source/cache/package/update concerns beyond loader/dependency planning. Record `TASK-060`.
- Session concurrency controls: AstrBot `session_waiter.py`, `session_lock.py`, and `active_event_registry.py` show multi-turn waiters, per-session locks, and reset/agent-stop registries. Record `TASK-061`.
- Realtime chat gateway: AstrBot `routes/live_chat.py` and `routes/open_api.py` show WebSocket connection sessions, audio frame assembly, queue subscriptions, OpenAPI chat DTOs, and response persistence need a web gateway boundary. Record `TASK-062`.
- File token/static asset serving: AstrBot `routes/file.py`, `routes/static_file.py`, and dashboard server asset selection show scoped downloads and dashboard assets need their own service boundary. Record `TASK-063`.

These are follow-up guardrails. The immediate implementation pointer remains `M7-R22-provider-registry-builtin-boundary` / `TASK-027`.

## TASK-027 Execution Update

`TASK-027` is completed. `ProviderRegistry` remains the public entry point, but registry internals are now split by responsibility:

- `registry/builtins.rs`: built-in chat, STT, TTS, embedding, and rerank registration.
- `registry/factory.rs`: per-capability factory trait object aliases.
- `registry/metadata.rs`: provider adapter metadata index, duplicate checks, and capability lookup helpers.
- `registry/errors.rs`: shared duplicate-registration and wrong-capability/unregistered-provider errors.

AstrBot comparison:

- This keeps the core idea from AstrBot `provider/register.py`: provider type names map to metadata and later instance construction.
- Rust differs intentionally by making capability buckets and factory signatures typed instead of relying on dynamic import/class checks from `provider/manager.py`.

Verification passed:

- `cargo fmt --all`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R23-agent-context-boundary` / `TASK-028`.

## TASK-028 Execution Update

`TASK-028` is completed. `astrbot-agent` now owns a dedicated context-window boundary instead of letting token budget, truncation, or compression policy grow inside pipeline stages or provider adapters:

- `context/window.rs`: typed `AgentContextWindow` over `ProviderContextMessage`.
- `context/token.rs`: pluggable token counter plus `ContextTokenBudget`.
- `context/truncation.rs`: recent-message and token-budget truncation policy.
- `context/compression.rs`: `AgentContextCompressor` trait and no-op placeholder.
- `context/manager.rs`: `ContextWindowManager` orchestration.
- `context/decorator.rs`: request decorator integration point for existing agent runner flows.

AstrBot comparison:

- This mirrors AstrBot `ContextManager`, `EstimateTokenCounter`, `ContextTruncator`, and `ContextCompressor` as explicit agent-context concerns.
- Rust keeps the first pass typed and provider-independent; LLM summary compression remains a future implementation behind `AgentContextCompressor`.

Verification passed:

- `cargo fmt --all`
- `cargo test -p astrbot-agent`
- `cargo test -p astrbot-pipeline`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

Next concrete task: `M7-R24-mcp-boundary` / `TASK-029`.

## TASK-029 Execution Update

`TASK-029` is completed. MCP now has its own Rust crate boundary instead of being implied by plugin tools or provider/tool-manager code:

- `crates/astrbot-mcp/src/config.rs`: MCP server config, transport resolution, active server filtering, client capability config.
- `client.rs`: lifecycle port, runtime state, and server capability snapshot.
- `tools.rs`: MCP tool descriptors, registrations, and tool-call DTOs.
- `resources.rs`: resource descriptors and server-scoped synthetic resource tool names.
- `prompts.rs`: prompt descriptors and server-scoped synthetic prompt tool names.
- `sampling.rs`: sampling request/message/content boundary and unsupported-mode guard.
- `elicitation.rs`: form/URL elicitation model, field types, and action parsing.
- `roots.rs`: roots allowlist policy and file URI model.

AstrBot comparison:

- AstrBot keeps MCP client lifecycle in `mcp_client.py`, subcapabilities in `mcp_subcapability_bridge.py`, resource/prompt synthetic tools in separate bridge modules, and runtime registration in `func_tool_manager.py`.
- Rust keeps those concepts, but makes the first pass a typed crate so plugin SDK, sandbox, provider adapters, and pipeline do not own MCP runtime state.

Verification passed:

- `cargo fmt --all`
- `cargo test -p astrbot-mcp`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

Next concrete task: `M7-R25-tool-schema-command-boundary` / `TASK-030`.

## TASK-030 Execution Update

`TASK-030` is completed. Tool schema and command management now have their own `astrbot-tool` crate boundary:

- `catalog.rs`: tool descriptors, source classification, deterministic catalog replacement, active views.
- `activation.rs`: disabled-tool and rename policy.
- `commands.rs`: command/group/sub-command descriptors, aliases, permission, parent signature composition.
- `conflicts.rs`: active tool and enabled command conflict detection.
- `schema/openai.rs`: OpenAI function-tool schema serializer.
- `schema/anthropic.rs`: Anthropic tool schema serializer.
- `schema/gemini.rs`: Gemini function declaration serializer and schema normalization.

AstrBot comparison:

- AstrBot `ToolSet` serializes OpenAI/Anthropic/Gemini schemas, while `FunctionToolManager` and command management also own activation, MCP registration, command toggles, renames, permissions, and conflict detection.
- Rust now separates those concerns before provider adapters or plugin registry grow tool-call parity behavior.

Verification passed:

- `cargo fmt --all`
- `cargo test -p astrbot-tool`
- `cargo test -p astrbot-plugin`
- `cargo test -p astrbot-provider`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

Next concrete task: `M7-R26-knowledge-base-boundary` / `TASK-031`.

## Additional Decoupling Audit - Post TASK-030

I rescanned the current Rust hotspots and compared them against AstrBot areas that are still not represented precisely enough by `TASK-031` through `TASK-063`.

Existing coverage check:

- KB/RAG, persistence migration, webhook security, provider media/streaming, multimodal, path/temp, T2I, observability, event routing, provider selection, conversation history, persona/skill prompt composition, cron/proactive jobs, subagents, computer-use, plugin dependency environment, platform API clients, quote/forward parsing, live feedback, dashboard auth/config/plugin/realtime/file serving are already recorded.
- Remaining large Rust files such as provider TTS adapters, `policy_config.rs`, plugin tests, and provider non-chat protocol files are mostly covered by existing pending tasks.

New decoupling space recorded:

- Long-term memory and active reply: AstrBot `builtin_stars/astrbot/long_term_memory.py` mixes per-session transcript retention, optional image captions, active-reply policy, and request mutation. Record `TASK-064`.
- External agent runners: AstrBot `core/agent/runners/{coze,dify,dashscope,deerflow}` have connector clients, thread/session mapping, stream event parsing, and final response mapping that should not become normal provider adapters. Record `TASK-065`.
- Real MCP transport and bridge runtime: `TASK-029` created typed MCP boundaries, but AstrBot `mcp_stdio_client.py`, `mcp_client.py`, and `mcp_subcapability_bridge.py` show separate stdio/HTTP transport, tolerant JSON-RPC framing, process supervision, reconnect, and tool bridge registration concerns. Record `TASK-066`.
- Backup job service: AstrBot `dashboard/routes/backup.py` and `core/backup/{exporter,importer}.py` combine chunked uploads, task progress, manifests, version compatibility, import/export orchestration, and downloads; Rust should keep this out of management route handlers. Record `TASK-067`.
- Platform outbound routing state: QQ Official, QQ webhook, DingTalk, and Lark adapters cache session scenes, last message IDs, sender bindings, and proactive-send route hints. Record `TASK-068`.
- Skill package lifecycle: AstrBot `core/skills/skill_manager.py` combines discovery, frontmatter parsing, prompt inventory rendering, local activation, sandbox-only cache, zip install/delete, and path sanitization; this is broader than persona prompt composition. Record `TASK-069`.
- Pipeline preprocess boundary: AstrBot `core/pipeline/preprocess_stage/stage.py` handles pre-ack reactions, media path mapping, and Record-to-STT text normalization before later policy/process stages. Record `TASK-070`.
- Internal tool provider source: AstrBot's FunctionToolManager and dashboard tool routes distinguish internal/built-in tools from plugin/MCP/subagent tools. Record `TASK-071`.
- ChatUI project domain: AstrBot `dashboard/routes/chatui_project.py` and conversation routes manage project ownership and session membership separately from message history. Record `TASK-072`.
- TTS streaming audio queue: AstrBot provider base and streaming TTS sources expose text/audio queue semantics distinct from file-based synthesis. Record `TASK-073`.

The immediate implementation pointer remains `M7-R26-knowledge-base-boundary` / `TASK-031`; these new tasks are growth guardrails for later parity work.

## Additional Decoupling Audit - Follow-Up Coverage

I rechecked the same AstrBot comparison against the newly recorded `TASK-064` through `TASK-069` and found four still-distinct boundary gaps:

- Pipeline preprocessing: AstrBot `core/pipeline/preprocess_stage/stage.py` sends platform pre-ack reactions, rewrites Record/Image paths, and converts voice records through STT before normal process/provider flow. Existing STT/provider and path/temp tasks do not define this stage boundary. Record `TASK-070`.
- Internal tool providers: AstrBot `core/provider/func_tool_manager.py`, `core/tools/kb_query.py`, and `dashboard/routes/tools.py` distinguish internal, plugin, and MCP tool origins, including internal-tool toggle policy. Existing tool schema/execution tasks do not define internal tool source registration. Record `TASK-071`.
- ChatUI projects: AstrBot `dashboard/routes/chatui_project.py` owns project CRUD, session membership, and creator ownership checks. Conversation-history tasks cover messages/history, but not project membership. Record `TASK-072`.
- Streaming TTS audio: AstrBot `core/provider/provider.py` and `core/provider/sources/genie_tts.py` define `support_stream` and `get_audio_stream` with text/audio queues. Existing provider media artifact and live-feedback tasks do not define a provider-neutral streaming TTS audio queue. Record `TASK-073`.

The immediate implementation pointer remains `M7-R26-knowledge-base-boundary` / `TASK-031`; `TASK-070` through `TASK-073` are additional guardrails for later parity work.

## TASK-031 Execution Update

`TASK-031` is completed. KB/RAG now has its own `astrbot-kb` crate instead of letting knowledge-base behavior grow inside pipeline stages, provider adapters, or agent request decorators:

- `types.rs`: KB, document, media, and chunk ID newtypes plus `KnowledgeChunk`.
- `document.rs`: knowledge-base profile, document, media, and stats DTOs.
- `parser.rs`: document parser port, parse result, media DTO, and plain-text parser.
- `chunking.rs`: chunker port and recursive character chunker with overlap.
- `embedding.rs`: embedding orchestration through `astrbot-provider` embedding traits.
- `vector_store.rs`: vector-store port plus in-memory test backend.
- `rank_fusion.rs`: reciprocal rank fusion.
- `retrieval.rs`: sparse retrieval port, in-memory sparse retriever, hybrid dense/sparse retrieval, and optional rerank provider integration.
- `formatter.rs`: retrieval-context formatting for future agent request decoration.

AstrBot comparison:

- This follows AstrBot's separate `knowledge_base` manager, models, parser, chunking, retrieval manager, sparse retriever, and rank-fusion modules.
- Rust keeps embedding/rerank providers in `astrbot-provider`; `astrbot-kb` consumes them as traits and does not own real DB/vector backend wiring in this first pass.

Verification passed:

- `cargo fmt --all`
- `cargo test -p astrbot-kb`
- `cargo test -p astrbot-provider`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R27-persistence-migration-boundary` / `TASK-032`.

## TASK-032 Execution Update

`TASK-032` is completed. `astrbot-storage` now has persistence, migration, stats, backup, repository, and SQLite planning boundaries before any real database backend is wired:

- `schema.rs`: typed schema/table/column descriptions, including AstrBot main DB v4 table groups such as conversations, platform stats, preferences, personas, platform sessions, API keys, cron jobs, ChatUI projects, command configs, and command conflicts.
- `migration.rs`: migration operation descriptors, migration state repository, in-memory migration state, declarative migration type, and idempotent migration runner.
- `stats.rs`: platform stats repository port and in-memory merge behavior for timestamp/platform/type keys.
- `repository.rs`: repository implementation descriptors carrying backend and schema identity.
- `sqlite.rs`: SQLite storage plan and AstrBot-inspired PRAGMA set (`journal_mode=WAL`, `synchronous=NORMAL`, cache/temp/mmap settings).
- `backup/manifest.rs`, `backup/export.rs`, `backup/import.rs`: backup manifest, export package, table dumps, file/directory records, import precheck, import mode/result, and version compatibility DTOs.

AstrBot comparison:

- This follows AstrBot `BaseDatabase`, `SQLiteDatabase.initialize`, `po.py` table model spread, migration marker helpers, platform stats upsert behavior, and backup manifest/import/export split.
- Rust keeps this as typed ports/plans for now; dashboard route handlers and runtime config IO do not own DB schema, migration, or backup archive policy.

Verification passed:

- `cargo fmt --all`
- `cargo test -p astrbot-storage`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R28-platform-webhook-security-boundary` / `TASK-033`.

## Additional Decoupling Audit - Post TASK-032

I rescanned the current Rust hotspots and compared them against AstrBot areas that are still not represented precisely enough by `TASK-033` through `TASK-073`.

Existing coverage check:

- Platform webhook/callback/long-connection work is already recorded as `TASK-033`.
- Provider media, streaming, multimodal, path/temp, T2I, observability, event routing, provider selection, conversation/history, persona/skill, cron/proactive jobs, subagent handoff, computer-use, plugin dependency environment, rich platform API clients, quote/forward parsing, live feedback, dashboard auth/config/plugin/realtime/file serving, memory, external runners, MCP transport, backup jobs, outbound routing, skill packages, pipeline preprocess, internal tools, ChatUI projects, and TTS streaming queues are already recorded.
- Large current Rust files such as `process_stage.rs`, platform tests, provider manager/TTS/protocol modules, plugin tests, and runtime policy config are already covered by existing pending tasks.

New decoupling space recorded:

- Storage schema catalog split: `crates/astrbot-storage/src/schema.rs` now contains table primitives plus many AstrBot main DB table groups. Record `TASK-074`.
- MCP wire primitives split: `crates/astrbot-mcp/src/types.rs` combines errors, names, URIs, JSON values, JSON schema, and pagination before real JSON-RPC transport lands. Record `TASK-075`.
- KB ingestion/indexing service: `TASK-031` covers parser/chunking/retrieval ports, but AstrBot `kb_mgr.py`, `kb_db_sqlite.py`, and `kb_helper.py` also show ingestion, metadata, indexing progress, and vector persistence orchestration. Record `TASK-076`.
- Agent hook/run-context boundary: `TASK-025`/`TASK-028` cover runner/decorator/context, but AstrBot `agent/hooks.py`, `run_context.py`, `message.py`, `response.py`, and `tool_image_cache.py` show side-channel lifecycle and tool-image cache concerns that should not grow inside the runner. Record `TASK-077`.

The immediate implementation pointer remains `M7-R28-platform-webhook-security-boundary` / `TASK-033`; these new tasks are growth guardrails for later storage, MCP, KB, and agent work.

## TASK-033 Execution Update

`TASK-033` is completed. Platform webhook and long-connection concerns now have shared typed boundaries under `astrbot-platform/src/adapters/common` instead of being pushed into `PlatformManager` or adapter event/message conversion:

- `security.rs`: signature input/verdict/verifier traits, WeCom-style SHA1 sorted-field verifier, encrypted webhook envelope, decoded payload DTO, and payload codec trait.
- `webhook.rs`: callback HTTP method/endpoint/request/response DTOs, callback handler/server traits, server state, and retry-event deduplicator.
- `long_connection.rs`: endpoint/state/reconnect policy, command/frame DTOs, request waiters, and long-connection client trait.
- `queue.rs`: inbound/outbound queue item DTOs, pending webhook response state, queue stats, queue trait, and in-memory queue store.
- `transport.rs`: `PlatformTransportKind::LongConnection` so future adapters can expose long-lived connection state without leaking implementation details into the manager.

AstrBot comparison:

- QQ Official webhook shows callback server lifecycle, validation opcode handling, and deduplication are transport concerns, while adapter event conversion remains separate.
- WeCom AI Bot shows SHA1 sorted-field verification, encrypted callback payloads, WebSocket long connection lifecycle, command waiters, heartbeat/retry policy, and per-session queues should not be mixed into generic platform registry/manager code.
- Rust keeps concrete AES/Ed25519 codecs, Axum route handlers, and real WeChat/QQ/WeCom adapters deferred; this pass defines the ports and DTOs that will keep those adapters small.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-platform`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

Next concrete task: `M7-R29-provider-test-media-fixture-boundary` / `TASK-034`.

## TASK-034 Execution Update

`TASK-034` is completed. Provider test media setup is now centralized in `crates/astrbot-provider/tests/support/media_fixture.rs`:

- `TempAudioFile`: creates temporary local audio files with explicit extension/content and removes them on drop.
- `TempOutputDir`: provides a unique output directory path for TTS providers and removes the directory tree on drop.
- `GeneratedAudioFile`: wraps generated provider output files and removes them on drop after assertions read the bytes.

Updated tests:

- TTS output directory fixtures: OpenAI TTS, Gemini TTS, Volcengine TTS, MiniMax TTS, and GSVI TTS tests.
- STT/audio input fixtures: OpenAI STT, Xinference STT, audio media conversion tests, and provider registry speech tests.
- Generated audio cleanup: provider registry TTS tests.

AstrBot comparison:

- AstrBot TTS sources write generated audio under `get_astrbot_temp_path()` with provider-specific filenames (`openai_tts_api`, `gemini_tts`, `minimax_tts_api`, `volcengine_tts`, `gsvi_tts_api`).
- Rust still keeps real provider output policy inside providers for now, but provider tests no longer own repeated temp path and cleanup mechanics.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

Next concrete task: `M7-R30-provider-media-artifact-boundary` / `TASK-035`.

## TASK-035 Execution Update

`TASK-035` is completed. Generated TTS artifact writing is now a provider media boundary instead of repeated filesystem logic inside every TTS adapter:

- `crates/astrbot-provider/src/media/artifact.rs`: owns default TTS output directory, safe media extension normalization, unique file naming, directory creation, audio file writes, empty-audio validation, and display path conversion.
- `crates/astrbot-provider/src/media/mod.rs`: exposes the internal provider media boundary to concrete adapters.
- OpenAI TTS, Gemini TTS, MiniMax TTS, Volcengine TTS, and GSVI TTS now delegate output path/write behavior to `GeneratedMediaArtifactWriter`.

AstrBot comparison:

- AstrBot TTS providers use the shared temp path policy from `core/utils/astrbot_path.py` while provider-specific sources keep protocol parsing and audio decoding local.
- Rust now follows the same split: generated media artifact policy is shared, while provider payloads, response parsing, MiniMax SSE audio collection, Gemini PCM-to-WAV conversion, and Volcengine `uid/reqid` generation stay in the adapters.

Post-task decoupling scan:

- Current large Rust files are still mostly test facades and provider/protocol/manager surfaces already represented in the backlog: `TASK-011`, `TASK-036`, `TASK-052`, `TASK-054`, `TASK-055`, `TASK-057`, `TASK-074`, `TASK-075`, and `TASK-076`.
- No new independent `TASK-078` is needed from this scan. The next highest-value split remains streaming parser/chunk/strategy separation in `TASK-036`.
- I also normalized stale Maestro plan flags to match task files: `TASK-033` and `TASK-034` are completed, while `TASK-011` and `TASK-058` remain pending.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R31-streaming-strategy-boundary` / `TASK-036`.

## TASK-036 Execution Update

`TASK-036` is completed. Provider streaming concerns now have a focused `astrbot-provider::streaming` boundary instead of being split between concrete protocol modules:

- `streaming/sse.rs`: owns provider-neutral SSE `data:` line extraction.
- `streaming/chunk.rs`: owns streamed text delta normalization for string/object/text-part shapes.
- `streaming/policy.rs`: owns unsupported streaming rejection policy while preserving existing provider error messages.

Updated behavior:

- OpenAI-compatible chat and MiniMax TTS protocol modules now use shared SSE extraction while keeping protocol-specific JSON parsing local.
- OpenAI-compatible chat now uses shared text delta normalization for streamed content chunks.
- Anthropic and Gemini now share the unsupported streaming policy instead of carrying adapter-local error construction.
- `RespondStage` and `MessageStream` remain delivery-only boundaries; provider transport parsing stays out of core and pipeline.

AstrBot comparison:

- AstrBot exposes `provider_settings.streaming_response` and `provider_settings.unsupported_streaming_strategy` in config, while providers expose streaming chat/TTS capabilities through provider abstractions.
- Rust now has the provider-side parsing and unsupported-policy boundary needed before adding full AstrBot `realtime_segmenting` / `turn_off` strategy parity and live agent feedback.

Post-task decoupling scan:

- No new independent `TASK-078` is needed from this pass. The existing backlog still covers the next visible growth points: multimodal preparation, path/temp cleanup, T2I rendering, observability, live-agent feedback, TTS streaming audio queues, storage schema catalog, MCP wire primitives, KB ingestion/index jobs, and agent hook/run-context side channels.
- The next highest-value split remains multimodal preparation in `TASK-037`, because AstrBot keeps image captioning, quoted media extraction, unsupported modality handling, and agent request decoration outside concrete providers.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test -p astrbot-pipeline`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R32-multimodal-preparation-boundary` / `TASK-037`.

## TASK-037 Execution Update

`TASK-037` is completed. Multimodal request preparation now has a focused `astrbot-agent::multimodal` boundary instead of growing inside `ProcessStage` or concrete provider adapters:

- `image_caption.rs`: owns `ImageCaptioner`, `ImageCaptionRequestDecorator`, caption config, and `ChatProviderImageCaptioner` for provider-backed image caption requests.
- `quoted_image.rs`: owns `QuotedImageAttachmentPolicy` for quoted/fallback image refs, dedupe, limits, and AstrBot-style attachment text.
- `capability_filter.rs`: owns provider modality support, image placeholder fallback, context image sanitization, and tool-use clearing policy.
- `mod.rs` and `lib.rs`: expose the boundary from `astrbot-agent` so pipeline assembly can later compose these as request decorators.

AstrBot comparison:

- AstrBot `_ensure_img_caption` calls a configured caption provider, inserts `<image_caption>...</image_caption>`, and clears `req.image_urls`; Rust models that as an agent request decorator.
- AstrBot `_append_quoted_image_attachment` appends `[Image Attachment in quoted message: path ...]`; Rust models that with a typed quoted-image policy separate from selected-text quote context.
- AstrBot `_modalities_fix` and `_sanitize_context_by_modalities` replace unsupported images and remove unsupported tools/context image parts; Rust centralizes that in a modality filter decorator.
- Provider adapters remain protocol serialization/parsing only, and `ProcessStage` did not gain image caption or modality fallback logic.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-agent`
- `cargo clippy -p astrbot-agent -- -D warnings`
- `cargo test -p astrbot-pipeline`
- `cargo test -p astrbot-provider`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R33-path-temp-artifact-boundary` / `TASK-038`.

## TASK-038 Execution Update

`TASK-038` is completed. Path policy and temporary artifact lifecycle now have typed boundaries instead of being scattered across provider/media code:

- `crates/astrbot-runtime/src/path_config.rs`: owns `RuntimePathConfig` and `RuntimePathLayout` for AstrBot-style `data/config`, `data/plugins`, `data/plugin_data`, `data/temp`, `data/webchat`, `data/t2i_templates`, `data/skills`, `data/site-packages`, `data/knowledge_base`, and `data/backups`.
- `RuntimeConfig.paths`: makes path defaults and overrides part of runtime config, with schema/UI metadata coverage.
- `crates/astrbot-storage/src/temp_artifact.rs`: owns `TempArtifactRoot`, safe artifact segments, artifact descriptors, cleanup policy, cleanup plans, and a filesystem cleaner.
- `crates/astrbot-provider/src/media/artifact.rs`: now derives default generated TTS output from `data/temp/generated_media/tts` through `TempArtifactRoot`, instead of direct `std::env::temp_dir` policy.

AstrBot comparison:

- AstrBot `astrbot_path.py` centralizes root/data/plugin/temp/backup/WebChat/T2I/skills paths; Rust now has a typed `RuntimePathLayout`.
- AstrBot `TempDirCleaner` scans temp files, removes oldest files under a max-size policy, and removes empty directories; Rust now has the same policy as a testable storage boundary.
- AstrBot backup export treats data directories and temp artifacts as managed surfaces; Rust now has the path and temp artifact contracts needed before backup/dashboard file APIs expand.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-storage`
- `cargo test -p astrbot-runtime`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-storage -- -D warnings`
- `cargo clippy -p astrbot-runtime -- -D warnings`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R34-t2i-render-boundary` / `TASK-039`.

## Additional Decoupling Audit - Post TASK-038

I rescanned the current Rust surfaces and compared them against AstrBot provider response, platform identity, session management, media IO, and update/maintenance references. No Rust source was changed in this pass.

Existing coverage check:

- T2I rendering, observability, event routing, provider selection, conversation/history, persona/skill, cron/proactive jobs, subagents, computer-use, plugin dependency environment, platform API clients, quote/forward parsing, live feedback, non-chat protocol DTOs, Xinference resolver, provider manager buckets, plugin tests, runtime policy config, TTS factory mapping, dashboard auth/config/plugin/realtime/file services, memory, external runners, MCP transport, backup jobs, outbound routing, skill packages, preprocess, internal tools, ChatUI projects, streaming TTS, storage schema, MCP wire types, KB ingestion, and agent hook/run-context are already recorded.
- `TASK-011` remains pending despite a stale `plan.json` flag; `TASK-037` is completed despite the stale plan flag. The plan was normalized to match task files.

New decoupling space recorded:

- Provider response metadata: AstrBot `LLMResponse` carries token usage, reasoning text/signatures, raw completion identity, stop/finish data, and tool-call payloads; Rust `ChatResponse` currently only carries `MessageChain`. Record `TASK-078`.
- Platform identity and membership: AstrBot `MessageMember`, `Group`, `AstrMessageEvent.role`, and adapter group/member lookups are richer than Rust `MessageSender` plus static `PermissionFilter`. Record `TASK-079`.
- Session rules and scoped preferences: AstrBot session management supports service/plugin/KB/provider preferences, UMO status, session groups, and batch updates beyond Rust's provider preference repository. Record `TASK-080`.
- Media normalization: provider image/audio paths still split file/base64/URL/data-URL conversion, MIME handling, and safe download rules across provider protocols, platform media, and WebChat attachments. Record `TASK-081`.
- Maintenance operations: AstrBot update routes handle release checks, project/dashboard update, pip install, and DB migration operations; Rust has migration/dependency pieces but no management operation boundary. Record `TASK-082`.

The immediate implementation pointer remains `M7-R34-t2i-render-boundary` / `TASK-039`; the new tasks are guardrails for later provider response, platform permission, dashboard session, media, and maintenance parity work.

## TASK-039 Execution Update

`TASK-039` is completed. T2I rendering now has its own `astrbot-render` crate instead of growing inside `RespondStage`, WebChat routes, or provider/media code:

- `crates/astrbot-render/src/lib.rs`: facade exports the render and template boundary.
- `crates/astrbot-render/src/t2i.rs`: owns `T2iRenderer`, typed render requests/results, strategy selection, render mode/format, artifact descriptors, and a local `TemplateRenderer` test implementation.
- `crates/astrbot-render/src/template.rs`: owns safe template names, built-in templates, user-template override behavior, and template CRUD/readback.

AstrBot comparison:

- AstrBot `HtmlRenderer` chooses network rendering and falls back to local rendering; Rust models that selection as `RenderStrategy` and keeps concrete network/local rasterizers behind the `T2iRenderer` trait.
- AstrBot `TemplateManager` uses a user-overrides-builtin template policy under `data/t2i_templates`; Rust keeps the same concept as a typed `TemplateCatalog` with path traversal protection.
- AstrBot local rendering mixes markdown parsing, font selection, image loading, raster drawing, and temp-file output in one file. Rust deliberately stops at the trait/template/artifact boundary in this pass, so future raster/browser or remote API implementations can plug in without coupling to pipeline/web layers.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-render`
- `cargo clippy -p astrbot-render -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R35-observability-trace-boundary` / `TASK-040`.

## Additional Decoupling Audit - Post TASK-039

I rescanned the current Rust workspace after the `astrbot-render` boundary and compared it against AstrBot T2I, shared IO, KB dashboard, metrics, and route-level reference extraction surfaces. No Rust source was changed in this pass.

Existing coverage check:

- `TASK-039` covers typed T2I renderer/template contracts, but not concrete network endpoint selection, local rasterization, markdown layout, or font/text measurement.
- `TASK-081` covers media input normalization, but not generic network download/TLS/proxy/progress/cache behavior shared by render, update, plugin, platform, and provider code.
- `TASK-031` and `TASK-076` cover KB retrieval and ingestion/index jobs, but not dashboard-facing KB CRUD, provider preflight, upload/import/from-URL tasks, progress maps, chunk CRUD, and stats.
- `TASK-040` covers trace/log/status observability and `TASK-078` covers provider response metadata, but not metrics sinks, installation identity, platform counts, token usage accounting, or dashboard stat aggregation.
- `TASK-051` covers live feedback and `TASK-071` covers internal tool providers, but route-local web-search citation/ref extraction remains a distinct transport-leak risk.

New decoupling space recorded:

- T2I implementation boundary: AstrBot splits `renderer.py`, `network_strategy.py`, `local_strategy.py`, and `template_manager.py`; Rust should split concrete network/local/markdown/font strategy modules behind `T2iRenderer`. Record `TASK-083`.
- Network download boundary: AstrBot `utils/io.py` and `utils/http_ssl.py` centralize downloads, TLS fallback, proxy/trust-env, progress, dashboard zip cache, and temp image output. Rust should not duplicate that across provider/platform/render/update flows. Record `TASK-084`.
- KB management API boundary: AstrBot `dashboard/routes/knowledge_base.py` owns CRUD, preflight, upload tasks, progress, chunks, stats, and debug retrieval above KB retrieval/ingestion. Record `TASK-085`.
- Metrics/usage boundary: AstrBot `utils/metrics.py`, agent stats, token usage persistence, platform stats, and dashboard `stat.py` should become a metrics/usage crate separate from logs/traces and provider parsers. Record `TASK-086`.
- Tool reference boundary: AstrBot `chat.py`, `live_chat.py`, and `open_api.py` duplicate web-search `<ref>` extraction from tool-call results; Rust should keep citation extraction in tool/agent services, not routes. Record `TASK-087`.

The immediate implementation pointer remains `M7-R35-observability-trace-boundary` / `TASK-040`; the new tasks are guardrails for later render, network IO, KB dashboard, stats, and citation parity work.

## TASK-040 Execution Update

`TASK-040` is completed. `astrbot-observability` now owns typed trace, log, and status boundaries instead of leaving lifecycle visibility inside runtime/platform/provider managers or WebChat routes:

- `crates/astrbot-observability/src/status_event.rs`: typed `ComponentKind`, `ComponentStatus`, `StatusSeverity`, `StatusEvent`, `StatusEventSink`, `NoopStatusEventSink`, and `InMemoryStatusCollector`.
- `crates/astrbot-observability/src/log_buffer.rs`: bounded in-memory log buffer, typed `LogEntry`, `LogLevel`, `LogSource`, and cursor-based snapshot reads.
- `crates/astrbot-observability/src/trace.rs`: typed `TraceSpan`, `TraceEvent`, `TraceSink`, `NoopTraceSink`, and `InMemoryTraceSink`.
- Runtime/provider/platform managers emit lifecycle events through a trait sink while keeping stop/restart behavior unchanged.
- Runtime restart preserves the configured observability sink through the restart state path.

AstrBot comparison:

- `core/utils/trace.py` mixes trace span creation, log broker publication, and trace-file logger output behind one helper. Rust now separates typed trace events from log buffering and manager lifecycle emission.
- `core/platform/manager.py` and `core/provider/manager.py` carry lifecycle visibility concerns that should stay outside route handlers. Rust now gives those managers a typed status-event sink instead of direct dashboard coupling.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-observability`
- `cargo test -p astrbot-runtime`
- `cargo test -p astrbot-provider`
- `cargo test -p astrbot-platform`
- `cargo clippy -p astrbot-observability -- -D warnings`
- `cargo clippy -p astrbot-runtime -- -D warnings`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo clippy -p astrbot-platform -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next concrete task: `M7-R36-event-bus-routing-boundary` / `TASK-041`.

## Additional Decoupling Audit - Post TASK-040

I rescanned the current Rust workspace after the observability boundary and compared it against AstrBot platform core, agent request construction, MCP surfaces, and remaining broad test facades. No Rust source was changed in this pass.

Existing coverage check:

- `TASK-041` already covers EventBus dispatch/routing/logging, so no new event-bus task is needed.
- `TASK-049`, `TASK-068`, and `TASK-079` cover richer platform API clients, outbound routing/bindings, and identity/membership, but they do not cover the current Rust `platform/core.rs` mixed contract/config/sink/validation file.
- `TASK-044`, `TASK-050`, `TASK-064`, `TASK-077`, and `TASK-081` cover persona, quote parsing, memory, hooks/run-context, and media normalization. They do not isolate the common agent request envelope/decorator/port composition boundary that those features will all plug into.
- `TASK-066` and `TASK-075` cover MCP transport/runtime and wire primitive splits, but the current `astrbot-mcp/src/tests.rs` remains a broad test facade that will grow with those implementations.

New decoupling space recorded:

- Platform core contract boundary: Rust `crates/astrbot-platform/src/core.rs` now mixes platform type constants, adapter traits, build context, `RecordingSink`, history test storage, sent-message DTOs, and ID validation. AstrBot keeps platform metadata, adapter base, register/manager, message/session, and concrete source adapters separate. Record `TASK-088`.
- Agent request decoration boundary: Rust `crates/astrbot-agent/src/request_decorator.rs` mixes event-to-provider request envelope policy, decorator trait/composition, provider preference/session/quote ports, and concrete context decorators. AstrBot's main agent and process sub-stages separate provider request construction, context windowing, modality fixes, plugin tool fixes, and hooks. Record `TASK-089`.
- Agent/MCP/tool test boundary: `astrbot-agent/src/tests.rs`, `astrbot-mcp/src/tests.rs`, and `astrbot-tool/src/tests.rs` are still broad test facades. Split them by behavior before request decorators, MCP runtime/transport, and tool references expand. Record `TASK-090`.

The immediate implementation pointer remains `M7-R36-event-bus-routing-boundary` / `TASK-041`. The new tasks are guardrails for platform contract cleanup, agent request composition, and test maintainability after the next event-bus work.
