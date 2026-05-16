# Reflection Log

## Round 0 - Audit Only

- Scope: workspace-level decoupling scan after `M7-R1-platform-module-split`.
- Strategy: identify large facade violations and map them back to AstrBot's registry/manager/capability boundaries.
- Result: no Rust source changes were made. Six refactor tasks were recorded in `plan.json`.
- Adjustment: continue migration only after runtime/provider/web/core/pipeline module splits are completed or explicitly deferred.
- Verification: JSON validity and formatting checks should run after workflow files are written.

## Round 1 - TASK-001 Runtime Facade Split

- Scope: `crates/astrbot-runtime/src/lib.rs`.
- Strategy: mechanical module split preserving public re-exports and behavior.
- Result: `lib.rs` now only declares modules and re-exports runtime public API. Runtime config, provider config, policy config, platform config, handle/lifecycle, assembly helpers, ports, defaults, config IO, and tests live in focused modules.
- Adjustment: provider registry/manager split should follow the same facade-preserving pattern, but should not mix in new provider parity behavior.
- Verification: `cargo check -p astrbot-runtime`, `cargo fmt --all --check`, `cargo test -p astrbot-runtime`, and `cargo test --workspace` passed.

## Round 2 - TASK-002 Provider Registry/Manager Split

- Scope: `crates/astrbot-provider/src/registry.rs`.
- Strategy: keep crate root re-exports compatible, then split provider constants, capability metadata, config structs, concrete factories, registry, and manager into separate modules.
- Result: `registry.rs` now focuses on provider type registration and construction dispatch; `manager.rs` owns configured provider instances and capability routing; `factories.rs` owns concrete provider adapter construction; `config.rs`, `capability.rs`, and `constants.rs` hold shared public types.
- Adjustment: next split should move provider public traits/request/response/mock providers out of `lib.rs` and split registry tests by capability. Do not add new provider parity in that split.
- Verification: `cargo check -p astrbot-provider`, `cargo fmt --all --check`, `cargo test -p astrbot-provider`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 3 - TASK-003 Provider Public Trait/Test Split And Second-Pass Audit

- Scope: `crates/astrbot-provider/src/lib.rs` and `crates/astrbot-provider/tests/provider_registry.rs`.
- Strategy: keep `lib.rs` as a compatibility facade, move public provider capability traits/DTOs/mock providers into focused modules, and split registry integration tests by capability.
- Result: public chat, STT, TTS, embedding, rerank, and mock definitions now live in `chat.rs`, `speech.rs`, `tts.rs`, `embedding.rs`, `rerank.rs`, and `mock.rs`; registry tests live under `tests/provider_registry/`.
- Adjustment: the next execution task is `M7-R4-web-core-pipeline-decoupling`. Fresh audit added second-pass items for runtime provider config/tests, provider config/factories, plugin SDK/sandbox, platform adapter transport/message conversion, large integration tests, and WebChat attachment/message-part services.
- AstrBot reference: keep mirroring AstrBot's separate message/component, platform adapter, pipeline stage, provider capability, Star handler, and sandbox tool boundaries, but express them as Rust modules and typed traits rather than growing crate roots.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-provider`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 4 - TASK-004 astrbot-web HTTP Boundary Split

- Scope: `crates/astrbot-web/src/lib.rs`.
- Strategy: keep `astrbot_web` public exports stable while moving HTTP DTOs, message-part conversion, error mapping, Axum handlers, server startup, and tests into focused modules.
- Result: `lib.rs` now only declares modules and re-exports public API; implementation lives in `dto.rs`, `message_parts.rs`, `error.rs`, `routes.rs`, `server.rs`, and `tests.rs`.
- AstrBot reference: this mirrors AstrBot WebChat's `platform/sources/webchat/message_parts_helper.py` as a dedicated conversion boundary, while keeping Rust HTTP handlers thin and typed.
- Adjustment: next split should target `crates/astrbot-core/src/message.rs`, because WebChat message-part conversion still depends on core message types that are currently concentrated in one domain file.
- Verification: `cargo test -p astrbot-web`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 5 - TASK-005 Core Message Domain Split

- Scope: `crates/astrbot-core/src/message.rs`.
- Strategy: replace the single domain file with a `message/` module tree and keep both `astrbot_core::message::*` and crate-root re-exports compatible.
- Result: message components, chains, session/sender metadata, sinks, result/stream types, provider request/context/tool DTOs, event state, and tests now live in focused modules under `crates/astrbot-core/src/message/`.
- AstrBot reference: this mirrors AstrBot's split between `core/message/components.py`, platform message/session/event models, and `message_event_result.py`, while preserving Rust's typed `MessageEvent` and `ProviderRequest` boundary.
- Adjustment: next split should target `crates/astrbot-pipeline/src/context.rs`, because the pipeline context still concentrates policy configs, ports, content safety, provider preference, quote policy, fallback config, and result decoration config.
- Verification: `cargo test -p astrbot-core`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 6 - TASK-006 Pipeline Context Split And Follow-Up Audit

- Scope: `crates/astrbot-pipeline/src/context.rs` plus post-first-pass hotspot scan.
- Strategy: keep `PipelineContext` as a facade/composition type and move stage policy configs, injected ports, provider preference storage, quote enrichment, content safety, and result decoration config into focused modules. Then rescan current Rust hotspots against AstrBot's pipeline, provider, platform, and Star/plugin boundaries.
- Result: pipeline context now lives under `crates/astrbot-pipeline/src/context/` with `mod`, `policy`, `session`, `provider_preference`, `quote`, `content_safety`, and `result` modules. New follow-up tasks were recorded for pipeline registry, provider HTTP helper, and CLI command boundaries.
- AstrBot reference: the context split follows AstrBot's thinner `pipeline/context.py` and stage-specific policy folders; the new backlog maps pipeline registry to `stage_order.py`/`bootstrap.py`, platform adapters to `platform/sources/*`, and plugin SDK/sandbox work to `star/*` plus tool execution boundaries.
- Adjustment: the highest-value next step is still plugin SDK/sandbox design before broad plugin loading or dashboard plugin management. Second-pass runtime/provider/platform/test splits should follow as separate, behavior-preserving tasks.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-pipeline`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed before this workflow-record update.

## Round 7 - TASK-009 Plugin SDK And Sandbox Boundary Split

- Scope: `crates/astrbot-plugin/src/lib.rs`.
- Strategy: preserve root re-exports and current `PluginRegistry` behavior, then split Star-inspired concerns into event, handler, registry, filters, manifest, sandbox, SDK, and tests modules. Add typed SDK/sandbox declarations without implementing dynamic plugin loading in the same change.
- Result: `astrbot-plugin` now exposes `PluginManifest`, `PluginCapability`, `PluginContext`, `PluginModule`, `PluginTestHarness`, `PluginPermission`, `ToolCapability`, `SandboxProfile`, and `SandboxRuntime` alongside the existing handler registry API. Command, regex, platform, permission, and session-kind filters live under `filter/`.
- AstrBot reference: the split keeps AstrBot Star's metadata/filter/priority model from `star_handler.py`, learns plugin context shape from `star/context.py`, and converts `astr_agent_tool_exec.py` sandbox capability checks into reusable Rust permission/profile types.
- Adjustment: real plugin loading, Python compatibility, config schema parsing, and sandbox execution remain deferred. The next structural pass should split runtime/provider capability config and factory/manager buckets before resuming broad parity.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-plugin`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 8 - TASK-007 Runtime Provider Config And Test Split

- Scope: `crates/astrbot-runtime/src/provider_config.rs` and `crates/astrbot-runtime/src/tests.rs`.
- Strategy: split by capability and behavior while preserving `astrbot_runtime::*` public re-exports and avoiding any provider/platform parity changes.
- Result: runtime provider config now has chat, speech, TTS, embedding, and rerank modules. Runtime tests now have behavior modules for message loop, provider config, policy, platform, lifecycle, and config IO with shared helpers kept in the facade test module.
- AstrBot reference: this follows AstrBot's provider manager capability buckets (`provider_insts`, `stt_provider_insts`, `tts_provider_insts`, `embedding_provider_insts`, `rerank_provider_insts`) while keeping Rust config DTOs explicit per capability.
- Adjustment: continue `M7-R6` with `TASK-008`, because provider-side config/factory/manager modules still aggregate capability buckets.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-runtime`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 9 - Additional Decoupling Audit

- Scope: current Rust hotspots after `TASK-007`/`TASK-009`, plus AstrBot provider/platform/plugin/tool/config references.
- Strategy: scan remaining large files and compare them with AstrBot's manager boundaries before continuing migration.
- Result: recorded `TASK-016` through `TASK-020` for provider test support, runtime task supervision/restart helpers, plugin loader/lifecycle/dependency/hot-reload boundaries, tool execution/sandbox capability boundaries, and config schema/default/env/UI-metadata boundaries.
- AstrBot reference: `platform/manager.py`, `provider/manager.py`, `star/star_manager.py`, `star/context.py`, `astr_agent_tool_exec.py`, and `config/default.py`.
- Adjustment: do not implement plugin loading or tool execution yet. Keep immediate execution on `TASK-008`, then platform/WebChat and provider HTTP helper splits, because they reduce current growth pressure before more parity work.
- Verification: this round changed workflow records only; JSON validity should be checked after writes.

## Round 10 - TASK-008 Provider Config/Factory/Manager Split

- Scope: `crates/astrbot-provider/src/config.rs`, `factories.rs`, and `manager.rs`.
- Strategy: keep provider public imports stable through facades, split capability-specific code into submodules, and avoid adding provider parity behavior.
- Result: provider config types live under `config/`; concrete builders live under `factories/`; `ProviderManagerConfigSet`, termination, and capability routing trait impls live under `manager/`.
- AstrBot reference: follows `provider/manager.py` capability buckets while keeping the Rust registry/factory boundary explicit.
- Adjustment: `M7-R6` structural provider cleanup is complete. Move to platform/WebChat media boundaries before WeChat/QQ parity and dashboard media APIs.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-provider`, `cargo clippy -p astrbot-provider -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 11 - TASK-010/TASK-012 Platform WebChat Boundary Split

- Scope: `crates/astrbot-platform/src/adapters/{onebot,webchat}` and `crates/astrbot-web/src`.
- Strategy: create adapter-local event/message submodules and thin WebChat history/attachment service boundaries without changing runtime entry points or HTTP behavior.
- Result: OneBot and WebChat adapters now own event construction and message conversion under their adapter module trees. WebChat HTTP history assembly is separated from routes, and attachment resolution has a typed service port for future dashboard media storage.
- AstrBot reference: follows `platform/sources/*` adapter-local event/message conversion and WebChat helper direction from `platform/sources/webchat/message_parts_helper.py`.
- Adjustment: platform/WebChat boundary gate is green. Next structural target is pipeline stage registry/order split.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-platform`, `cargo test -p astrbot-web`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 12 - Additional Decoupling Audit

- Scope: current Rust hotspots after adapter/WebChat boundary splits, with a fresh comparison against AstrBot provider sources, platform transports, main-agent/tool-loop code, database/backup layers, and dashboard management needs.
- Strategy: do not change Rust source. Record only backlog items that are not already covered by `TASK-011` through `TASK-020`.
- Result: recorded `TASK-021` through `TASK-026` for WebChat test modularization, provider protocol DTO/parser extraction, platform transport/session/media boundaries, storage repository ports, an agent runner/request decoration boundary, and dashboard management API separation.
- AstrBot reference: `platform/sources/aiocqhttp/*`, `platform/sources/qqofficial/*`, `provider/sources/*`, `astr_main_agent.py`, `agent/runners/tool_loop_agent_runner.py`, `db/sqlite.py`, `backup/exporter.py`, and dashboard-facing manager surfaces.
- Adjustment: keep the immediate implementation pointer on `M7-R8-pipeline-registry-boundary` / `TASK-013`; treat the new items as later growth guards unless platform transport, dashboard, or agent parity is explicitly prioritized.
- Verification: workflow records only; JSON validity should be checked after writes.

## Round 13 - TASK-013 Pipeline Registry Boundary Split

- Scope: `crates/astrbot-pipeline/src/registry.rs` and registry unit tests.
- Strategy: keep `PipelineStageRegistry` and public stage constants import-compatible, then split stage order constants, built-in registration, registered entry storage, and tests into focused submodules.
- Result: `registry.rs` now owns the public facade and core registration behavior; `registry/order.rs`, `registry/builtins.rs`, and `registry/entry.rs` own the separated responsibilities. Registry tests are grouped under `registry/tests/`.
- AstrBot reference: this follows AstrBot's `stage_order.py` and `bootstrap.py` split while preserving Rust's typed registry and `DefaultPipelineBuilder` path.
- Adjustment: next structural target is `M7-R9-provider-http-helper-boundary` / `TASK-014`, because provider adapters repeat base URL, header/auth, error parsing, and response helper logic.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-pipeline`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 14 - TASK-014 Provider HTTP Helper Boundary Split

- Scope: `crates/astrbot-provider/src/http*` and concrete provider adapters for chat, embedding, STT, TTS, and rerank.
- Strategy: extract only repeated HTTP mechanics: base/path joining, client timeout/header setup, bearer/API-key/custom header assembly, and common JSON error extraction. Keep provider-specific payload and response mapping local.
- Result: provider adapters now use `http/auth.rs`, `http/client.rs`, `http/error.rs`, and `http/url.rs`; concrete providers retain their protocol DTOs and adapter trait implementations.
- AstrBot reference: follows repeated patterns across `provider/sources/*` without duplicating AstrBot's source-level coupling.
- Adjustment: next concrete implementation target is `M7-R10-cli-entrypoint-boundary` / `TASK-015`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-provider`, `cargo clippy -p astrbot-provider -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 15 - Additional Decoupling Audit

- Scope: current Rust hotspots after provider HTTP helper extraction plus AstrBot agent context, MCP, function-tool, command-management, knowledge-base, DB/backup, and webhook platform references.
- Strategy: avoid source changes; only record boundary candidates that are not already covered by `TASK-011` through `TASK-026`.
- Result: recorded `TASK-027` through `TASK-033` for provider registry built-ins, agent context window/compression, MCP capabilities, tool schema/command catalog, KB/RAG, persistence migration/backup, and platform webhook security.
- AstrBot reference: `agent/context/*`, `agent/mcp_client.py`, `agent/mcp_subcapability_bridge.py`, `provider/func_tool_manager.py`, `star/command_management.py`, `knowledge_base/*`, `db/*`, `backup/*`, and WeChat/QQ webhook server/crypto modules.
- Adjustment: keep immediate execution pointer on `M7-R10-cli-entrypoint-boundary`; treat the new tasks as growth guards before main-agent, MCP, KB, persistence, and real WeChat/QQ parity expand.
- Verification: workflow records only; JSON validity should be checked after writes.

## Round 16 - TASK-015 CLI Entrypoint Boundary Split

- Scope: `crates/astrbot-cli/src/main.rs`.
- Strategy: keep command behavior unchanged while moving argument parsing, command dispatch/handlers, WebChat server launcher, and tests into focused modules.
- Result: `main.rs` now only wires `args::parse_command()` to `commands::execute()`. `args.rs`, `commands/{mod,init,run,smoke}.rs`, `webchat_server.rs`, and `tests.rs` own the former CLI responsibilities.
- AstrBot reference: Rust keeps a thin CLI lifecycle adapter instead of copying broad AstrBot startup/dashboard coupling into the entrypoint.
- Adjustment: next structural target is `M7-R11-provider-test-support-boundary` / `TASK-016`, because provider parity tests repeat HTTP server/request-capture helpers.
- Verification: `cargo test -p astrbot-cli`, `cargo fmt --all --check`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 17 - Additional Decoupling Audit

- Scope: current Rust hotspots after shared provider test support was introduced, plus AstrBot provider media, main-agent multimodal, path/temp, T2I, and observability references.
- Strategy: do not change source code; verify whether `TASK-016` is truly complete, then record only additional boundary candidates that are not already covered by `TASK-017` through `TASK-033`.
- Result: `TASK-016` stays `in_progress` because multiple provider tests still carry local HTTP fixture helpers. Recorded `TASK-034` through `TASK-040` for provider test support completion, provider generated media artifacts, streaming strategy, multimodal request preparation, path/temp cleanup, T2I rendering, and observability/log/trace boundaries.
- AstrBot reference: `provider/provider.py`, `provider/sources/*_tts*.py`, `astr_main_agent.py`, `utils/astrbot_path.py`, `utils/temp_dir_cleaner.py`, `utils/t2i/*`, and `utils/trace.py`.
- Adjustment: immediate pointer remains `M7-R11-provider-test-support-boundary`; do not resume provider/platform parity until the current support-boundary work is either completed or explicitly deferred.
- Verification: workflow records only; JSON validity should be checked after writes.

## Round 18 - TASK-016 Provider Test Support Boundary Split

- Scope: provider integration tests under `crates/astrbot-provider/tests`.
- Strategy: keep provider-specific protocol assertions in each adapter test, but move socket-level HTTP fixtures, request capture, response sequencing, and shared header assertions into `tests/support`.
- Result: `TASK-016` is complete. `tests/support/http_server.rs` now owns `serve_once`, `serve_sequence`, `TestResponse`, content-length-aware request capture, and response writing. `tests/support/captured_request.rs` owns shared captured request header assertions. Adapter tests for Anthropic, Gemini, OpenAI-compatible chat, embedding, rerank, STT, TTS, and provider registry coverage now import shared helpers instead of local HTTP harnesses.
- AstrBot reference: this follows the repeated provider source/test shape in `provider/sources/*`, while using Rust shared test helpers to keep parity tests focused on behavior.
- Adjustment: next implementation pointer moves to `M7-R12-runtime-task-supervision-boundary` / `TASK-017`. `TASK-034` is narrowed to provider test media fixture and temp path helpers; it no longer covers HTTP fixture migration.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-provider`, `cargo clippy -p astrbot-provider -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 19 - Additional Decoupling Audit

- Scope: current Rust hotspots after `TASK-016`, plus AstrBot event bus, provider selection, conversation/history, persona/skills, cron, and subagent orchestration references.
- Strategy: do not change Rust source. Record only boundaries not already covered by `TASK-017` through `TASK-040`, and keep the immediate execution pointer on runtime handle supervision.
- Result: recorded `TASK-041` through `TASK-046` for event bus dispatch/routing/logging, provider selection state/change hooks, conversation and platform message history domain services, persona/skill prompt composition, cron/proactive agent jobs, and subagent handoff orchestration.
- AstrBot reference: `event_bus.py`, `provider/manager.py`, `conversation_mgr.py`, `platform_message_history_mgr.py`, `persona_mgr.py`, `skills/skill_manager.py`, `cron/manager.py`, and `subagent_orchestrator.py`.
- Adjustment: these tasks are growth guards. They should not preempt `TASK-017` unless the next feature needs event routing, provider management APIs, persistent conversation history, persona/skills, cron, or subagent handoff.
- Verification: workflow records only; JSON validity should be checked after writes.

## Round 20 - TASK-017 Runtime Task Supervision Boundary Split

- Scope: `crates/astrbot-runtime/src/handle.rs`.
- Strategy: preserve `astrbot_runtime::{AstrbotRuntime, RuntimeHandle}` and runtime behavior while splitting public runtime types, background task supervision, restart state transfer, and mock/sent-message helper code into separate modules.
- Result: `handle/mod.rs` is now the facade; `handle/runtime.rs` owns public runtime/handle types and start/stop; `handle/supervisor.rs` owns event-bus/platform task spawn and abort/join mapping; `handle/restart.rs` owns provider preference restart state; `handle/testing.rs` owns mock platform emit and sent-message readback helpers. Added focused supervisor tests for cancelled tasks, task-result errors, and join failures.
- AstrBot reference: `core_lifecycle.py` and `platform/manager.py` show task wrapper/cancellation/error concerns separate from restart/reload and manager termination. Rust keeps the same lifecycle idea but uses typed task sets and Rust error mapping.
- Adjustment: next implementation pointer moves to `M7-R13-plugin-loader-lifecycle-boundary` / `TASK-018`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-runtime`, `cargo clippy -p astrbot-runtime -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 21 - TASK-018 Plugin Loader Lifecycle Boundary

- Scope: `crates/astrbot-plugin/src`.
- Strategy: define typed Rust boundary surfaces for loader metadata, dependency plans, lifecycle state, hot reload decisions, state store records, platform extensions, and web API route descriptors. Keep dynamic native/Python loading explicitly deferred.
- Result: added `loader/` modules for metadata, dependency, lifecycle, store, and hot reload; added `extension/` modules for platform and web API descriptors; expanded crate re-exports and tests. `PluginLoader` can discover manifests, transition loaded/active/disabled/unloaded states, and build plugin contexts from metadata without coupling to handler registry or sandbox execution.
- AstrBot reference: `star/star_manager.py` mixes discovery, metadata parsing, dependency install, dynamic import, hot reload, enable/disable, uninstall, tool cleanup, platform adapter cleanup, and web API registration. Rust now has separate typed surfaces for these concerns before Star parity expands.
- Adjustment: next implementation pointer moves to `M7-R14-tool-execution-sandbox-boundary` / `TASK-019`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-plugin`, `cargo clippy -p astrbot-plugin -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 22 - TASK-019 Tool Execution Sandbox Boundary

- Scope: `crates/astrbot-plugin/src/tool` plus plugin public re-exports and tests.
- Strategy: keep concrete tool execution deferred, but split declarations, local/MCP/handoff/background kind metadata, execution request/result/status, executor traits, and sandbox capability checks into a typed boundary.
- Result: `PluginToolDeclaration`, `PluginToolKind`, `HandoffToolTarget`, `BackgroundTaskPolicy`, `ToolCapabilityDecision`, `ToolExecutionRequest`, `ToolExecutionResult`, `ToolExecutionStatus`, `ToolExecutor`, and `SandboxedToolExecutor` now live under a focused `tool/` module tree.
- AstrBot reference: `astr_agent_tool_exec.py` shows local tool calls, MCP, handoff, background tasks, wake-up behavior, and sandbox checks as a high-coupling area; Rust now has typed seams before implementing real execution.
- Adjustment: next implementation pointer moves to `M7-R15-config-schema-boundary` / `TASK-020`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-plugin`, `cargo test -p astrbot-pipeline`, `cargo clippy -p astrbot-plugin -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 23 - Additional Decoupling Audit

- Scope: current Rust hotspots after `TASK-019`, plus AstrBot computer-use, pip installer, rich platform adapter, quoted-message parser, and agent run utility references.
- Strategy: avoid Rust source changes; record only boundaries not already covered by `TASK-020` through `TASK-046`.
- Result: recorded `TASK-047` through `TASK-051` for computer-use runtime, plugin dependency environment, platform API clients, quote/forward parsing, and live-agent feedback orchestration.
- AstrBot reference: `computer/computer_client.py`, `computer/computer_tool_provider.py`, `utils/pip_installer.py`, `platform/sources/misskey/misskey_api.py`, `utils/quoted_message/chain_parser.py`, and `astr_agent_run_util.py`.
- Adjustment: these tasks are growth guards. They should not preempt `TASK-020` unless the next feature needs computer-use, real plugin dependency installation, rich platform API clients, quote/multimodal parsing, or live streaming/TTS feedback.
- Verification: workflow records only; JSON validity should be checked after writes.

## Round 24 - TASK-020 Config Schema Boundary

- Scope: `crates/astrbot-runtime/src/config.rs`, `config_io.rs`, root defaults facade, new `config/` submodules, public runtime re-exports, and runtime config tests.
- Strategy: keep `RuntimeConfig` and current JSON behavior stable while moving defaults, env lookup, secret redaction, migration/default-merge policy, schema metadata, and UI metadata into focused config submodules.
- Result: runtime config now exposes `RuntimeEnvConfigSource`, `SecretValue`, `RuntimeConfigMigrationPlan`, `RuntimeConfigSchema`, `ConfigFieldSchema`, `ConfigUiMetadata`, and related helpers without putting those responsibilities inside the DTO or config IO code.
- AstrBot reference: `config/default.py` and `config/astrbot_config.py` show broad defaults, schema-to-default conversion, integrity checking, UI metadata, and secret fields; Rust keeps those ideas but uses typed modules and re-exports.
- Adjustment: next implementation pointer moves to `M7-R16-web-test-boundary` / `TASK-021`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-runtime`, `cargo clippy -p astrbot-runtime -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 25 - TASK-021 WebChat Test Boundary

- Scope: `crates/astrbot-web/src/tests.rs` and new `crates/astrbot-web/src/tests/*` modules.
- Strategy: keep HTTP behavior unchanged while splitting submit route tests, message-part validation, history readback, live server behavior, and shared support helpers.
- Result: WebChat tests now use a facade plus `support`, `submit`, `message_parts`, `history`, and `server` modules. Shared setup is centralized, and future auth/streaming/storage tests have clear module destinations.
- AstrBot reference: this preserves the WebChat message-part helper concept from `platform/sources/webchat/message_parts_helper.py` without letting all HTTP coverage accumulate in one file.
- Adjustment: next implementation pointer moves to `M7-R17-provider-protocol-boundary` / `TASK-022`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-web`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 26 - TASK-022 Provider Protocol Boundary

- Scope: provider adapter protocol DTOs and parsers for OpenAI-compatible chat, Gemini chat, Anthropic chat, MiniMax TTS SSE audio, and Xinference STT/Rerank list/launch/response parsing.
- Strategy: preserve adapter behavior and public crate exports while moving protocol wire shapes into `crates/astrbot-provider/src/protocol`.
- Result: added protocol modules for shared SSE parsing, OpenAI-compatible chat, Gemini chat, Anthropic chat, MiniMax TTS, and Xinference. Concrete adapters now orchestrate config, URL/header construction, HTTP calls, media/artifact behavior, and trait methods.
- AstrBot reference: `openai_source.py`, `gemini_source.py`, `minimax_tts_api_source.py`, `xinference_stt_provider.py`, and `xinference_rerank_source.py` show provider-local protocol concerns; Rust keeps that locality but separates it from lifecycle orchestration.
- Adjustment: next implementation pointer moves to `M7-R18-platform-transport-boundary` / `TASK-023`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-provider`, `cargo clippy -p astrbot-provider -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 27 - Additional Decoupling Audit

- Scope: current Rust hotspots after `TASK-022`, plus AstrBot non-chat provider and Xinference references.
- Strategy: record only provider decoupling spaces not already covered by `TASK-023` through `TASK-051`.
- Result: recorded `TASK-052` for remaining non-chat provider protocol DTO/parsers and `TASK-053` for shared Xinference model resolver lifecycle.
- AstrBot reference: `openai_embedding_source.py`, `gemini_embedding_source.py`, `openai_tts_api_source.py`, `gemini_tts_source.py`, `volcengine_tts.py`, `whisper_api_source.py`, `bailian_rerank_source.py`, `vllm_rerank_source.py`, `xinference_stt_provider.py`, and `xinference_rerank_source.py`.
- Adjustment: these tasks are guardrails; they should not preempt `TASK-023` unless the next provider parity work touches those adapters.
- Verification: workflow JSON validity and task counts should be checked after writes.

## Round 28 - TASK-023 Platform Transport Boundary

- Scope: shared platform transport/media boundary types plus OneBot session and transport modules.
- Strategy: keep `PlatformRegistry` and `PlatformManager` as the only runtime entry points while introducing typed surfaces for future network lifecycle, reconnect state, session metadata, and media upload.
- Result: added `adapters/common/transport.rs`, `adapters/common/media.rs`, `adapters/onebot/session.rs`, and `adapters/onebot/transport.rs`. `OneBotPlatform::run` and `terminate` now delegate through `OneBotTransport`; current in-process submit behavior is unchanged.
- AstrBot reference: `platform/manager.py` supervises platform tasks, `aiocqhttp_platform_adapter.py` owns reverse WebSocket lifecycle, and `qqofficial_platform_adapter.py` shows session scene/message ID plus media upload complexity that should not enter adapter message conversion.
- Adjustment: next implementation pointer moves to `M7-R19-storage-boundary` / `TASK-024`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-platform`, `cargo clippy -p astrbot-platform -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 29 - TASK-024 Storage Repository Boundary

- Scope: `astrbot-storage`, WebChat history readback, provider preference, and runtime restart state.
- Strategy: introduce storage ports before adding persistent backends, keeping current in-memory behavior but moving history/preference persistence concerns behind repository traits.
- Result: added conversation history, attachment, provider preference, and config snapshot repositories with in-memory implementations. WebChat history now reads through `ConversationHistoryRepository`, provider preference delegates to `ProviderPreferenceRepository`, and restart state capture/restore awaits the storage-backed preference port.
- AstrBot reference: `db/sqlite.py`, `backup/exporter.py`, and `backup/importer.py` show persistence/backup as a broad concern; Rust now has ports that keep platform, pipeline, web, and runtime from binding directly to a concrete database.
- Adjustment: next implementation pointer moves to `M7-R20-agent-runner-boundary` / `TASK-025`.
- Verification: `cargo fmt --all`, targeted package tests for storage/pipeline/platform/web/runtime, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` passed.

## Round 30 - Additional Decoupling Audit

- Scope: current Rust hotspots after `TASK-024`, plus AstrBot provider manager, config defaults, and plugin/provider growth references.
- Strategy: record only boundary candidates not already covered by `TASK-025` through `TASK-053`.
- Result: recorded `TASK-054` through `TASK-057` for reusable provider manager bucket assembly/accessors, plugin test modularization, runtime policy config module split, and TTS factory option normalization.
- AstrBot reference: `provider/manager.py` confirms bucket/default-selection pressure, `config/default.py` shows `platform_settings` will grow beyond current runtime policy config, and provider source spread confirms TTS options should not remain centralized in one factory file.
- Adjustment: these are guardrails behind the immediate agent-runner and management/API boundary work unless the next feature touches those areas directly.
- Verification: workflow JSON validity and task counts should be checked after writes.

## Round 31 - TASK-025 Agent Runner Boundary

- Scope: new `astrbot-agent` crate plus pipeline provider fallback integration.
- Strategy: move provider request envelope construction, request decoration, fallback error handling, and provider chat execution behind an agent runner while preserving current pipeline behavior.
- Result: added `AgentRunner`, `ChatAgentRunner`, `AgentFallbackPolicy`, request decorator traits/adapters, `PersonaPromptDecorator`, and tool-loop policy placeholders. `ProviderStage` now delegates provider fallback through `ChatAgentRunner`; pipeline-specific ports are adapted by private wrapper structs.
- AstrBot reference: `astr_main_agent.py` owns request decoration/persona/context/tool-loop concerns, while `tool_loop_agent_runner.py` owns agent execution state. Rust now has a typed crate boundary for those concerns before main-agent parity expands.
- Adjustment: next implementation pointer moves to `M7-R21-management-api-boundary` / `TASK-026`.
- Verification: `cargo fmt --all`, `cargo test -p astrbot-agent`, `cargo test -p astrbot-pipeline`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` passed.

## Round 32 - TASK-026 Management API Boundary

- Scope: `crates/astrbot-web/src/management`, `crates/astrbot-web/src/server.rs`, and manager facade reads.
- Strategy: keep WebChat transport routes focused on chat submit/history while adding a separate management router for dashboard/provider/platform/plugin status snapshots.
- Result: `astrbot-web` now exposes management API state, router, status, provider, platform, and plugin DTO modules. Server helpers can serve management routes independently, and `PlatformManager` exposes count/list helpers without leaking adapter storage.
- AstrBot reference: provider/platform/plugin manager surfaces in AstrBot feed dashboard status, but Rust keeps this first pass read-only and snapshot-oriented so dashboard code does not reach into `EventBus`, `Pipeline`, or adapter internals.
- Adjustment: next implementation pointer moves to `M7-R22-provider-registry-builtin-boundary` / `TASK-027`; dashboard mutation APIs remain deferred until provider selection/config/log boundaries are in place.
- Verification: `cargo fmt --all`, `cargo test -p astrbot-web`, `cargo test -p astrbot-platform`, `cargo test -p astrbot-runtime`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` passed.

## Round 33 - Additional Decoupling Audit

- Scope: current Rust hotspots after `TASK-026`, plus AstrBot dashboard auth/config/OpenAPI/live-chat/file routes, plugin market/update routes, and session concurrency utilities.
- Strategy: avoid Rust source changes; record only boundaries not already covered by `TASK-027` through `TASK-057`.
- Result: recorded `TASK-058` through `TASK-063` for management auth/API-key scopes, dashboard config mutation and UMOP routing, plugin marketplace/update source handling, session waiter/lock/active-event interruption, realtime chat/OpenAPI gateway, and file-token/static asset serving.
- AstrBot reference: `dashboard/server.py`, `dashboard/routes/auth.py`, `api_key.py`, `open_api.py`, `config.py`, `plugin.py`, `update.py`, `live_chat.py`, `file.py`, `static_file.py`, `core/umop_config_router.py`, `core/star/updator.py`, and `core/utils/{session_waiter,session_lock,active_event_registry}.py`.
- Adjustment: these tasks are guardrails. They should not preempt `TASK-027` unless the next feature touches dashboard auth/config/OpenAPI, plugin marketplace/update, multi-turn session waits, realtime chat, or file serving.
- Verification: workflow records only; JSON validity and task counts should be checked after writes.

## Round 34 - TASK-027 Provider Registry Builtin Boundary

- Scope: `crates/astrbot-provider/src/registry.rs` and new `registry/*` submodules.
- Strategy: preserve `ProviderRegistry` public API and behavior while moving built-in provider registration, factory trait object aliases, metadata indexing, duplicate checks, and registry error construction into focused modules.
- Result: `registry.rs` is now a facade over registration/build methods. `builtins.rs` owns deterministic built-in registration, `factory.rs` owns per-capability factory types, `metadata.rs` owns adapter metadata lookup, and `errors.rs` owns shared duplicate/missing-factory errors.
- AstrBot reference: mirrors `provider/register.py` provider type metadata map and `provider/manager.py` capability-bucket construction, but uses Rust trait object factories and typed capabilities instead of dynamic imports.
- Adjustment: next implementation pointer moves to `M7-R23-agent-context-boundary` / `TASK-028`.
- Verification: `cargo fmt --all`, `cargo test -p astrbot-provider`, `cargo clippy -p astrbot-provider -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 35 - TASK-028 Agent Context Boundary

- Scope: new `crates/astrbot-agent/src/context/*` modules plus agent facade exports and focused context tests.
- Strategy: follow AstrBot's explicit context manager/token counter/truncator/compressor separation, but keep the Rust first pass provider-independent and composable through request decorators.
- Result: added `AgentContextWindow`, `AgentTokenCounter`, `ApproximateTokenCounter`, `ContextTokenBudget`, `ContextTruncationPolicy`, `AgentContextCompressor`, `NoopContextCompressor`, `ContextWindowManager`, and `ContextWindowRequestDecorator`. Pipeline stages and concrete providers remain unchanged.
- AstrBot reference: `agent/context/manager.py`, `compressor.py`, `truncator.py`, and `token_counter.py` confirm context budget and compression are agent-context concerns, not provider or pipeline responsibilities.
- Adjustment: next implementation pointer moves to `M7-R24-mcp-boundary` / `TASK-029`.
- Verification: `cargo fmt --all`, `cargo test -p astrbot-agent`, `cargo test -p astrbot-pipeline`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` passed.

## Round 36 - TASK-029 MCP Boundary

- Scope: new `crates/astrbot-mcp` crate, workspace membership, and typed MCP boundary tests.
- Strategy: follow AstrBot's separation of MCP client lifecycle, resource/prompt bridges, sampling, elicitation, roots, and tool registration, but keep Rust's first pass as a dependency-light typed boundary without owning real network transport.
- Result: added MCP config, lifecycle port/state snapshots, tool/resource/prompt descriptors, synthetic bridge tool naming, sampling request guard, elicitation request/action model, and roots allowlist policy. `astrbot-plugin` keeps only its existing lightweight `PluginToolKind::Mcp` declaration.
- AstrBot reference: `mcp_client.py`, `mcp_subcapability_bridge.py`, `mcp_resource_bridge.py`, `mcp_prompt_bridge.py`, and `func_tool_manager.py` show the surfaces that should remain separate instead of landing in providers or pipeline.
- Adjustment: next implementation pointer moves to `M7-R25-tool-schema-command-boundary` / `TASK-030`.
- Verification: `cargo fmt --all`, `cargo test -p astrbot-mcp`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` passed.

## Round 37 - TASK-030 Tool Schema And Command Boundary

- Scope: new `crates/astrbot-tool` crate, workspace membership, and typed catalog/schema/command tests.
- Strategy: keep provider schema formatting, activation policy, command descriptors, and conflict detection out of provider adapters, plugin registry internals, and `ProviderRequest`.
- Result: added `ToolDescriptor`, `ToolCatalog`, `ToolActivationPolicy`, `CommandDescriptor`, command permissions/types, active tool and command conflict detection, plus OpenAI/Anthropic/Gemini serializers.
- AstrBot reference: `agent/tool.py` owns ToolSet provider-schema serializers, `func_tool_manager.py` owns function/MCP tool catalog behavior, and `command_management.py` owns command toggles, renames, permissions, and conflict detection. Rust now gives those concerns a typed crate boundary.
- Adjustment: next implementation pointer moves to `M7-R26-knowledge-base-boundary` / `TASK-031`.
- Verification: `cargo fmt --all`, `cargo test -p astrbot-tool`, `cargo test -p astrbot-plugin`, `cargo test -p astrbot-provider`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` passed.

## Round 38 - Additional Decoupling Audit

- Scope: current Rust hotspots after `TASK-030`, plus AstrBot long-term memory, external agent runners, MCP stdio/bridge runtime, backup routes, rich platform outbound state, and skill manager surfaces.
- Strategy: avoid Rust source changes; record only boundaries not already covered precisely by `TASK-031` through `TASK-063`.
- Result: recorded `TASK-064` through `TASK-073` for long-term memory/active reply, external agent runner connectors, MCP transport bridge runtime, backup job services, platform outbound routing state, skill package lifecycle, pipeline preprocess, internal tool providers, ChatUI projects, and TTS streaming audio queues.
- AstrBot reference: `builtin_stars/astrbot/long_term_memory.py`, `core/agent/runners/*`, `core/agent/mcp_stdio_client.py`, `core/agent/mcp_subcapability_bridge.py`, `dashboard/routes/backup.py`, `core/backup/{exporter,importer}.py`, `platform/sources/{qqofficial,dingtalk,lark}`, `core/skills/skill_manager.py`, `pipeline/preprocess_stage/stage.py`, `dashboard/routes/tools.py`, `dashboard/routes/chatui_project.py`, and provider TTS streaming sources.
- Adjustment: these tasks are guardrails behind the immediate `M7-R26-knowledge-base-boundary` / `TASK-031` pointer unless the next feature touches memory, external runners, MCP runtime, backups, platform proactive routing, skill package management, preprocess, internal tools, ChatUI projects, or streaming TTS.
- Verification: workflow records only; JSON validity and task counts should be checked after writes.

## Round 39 - TASK-070 Through TASK-073 Sync

- Scope: synchronize the follow-up guardrails already present in `plan.json`, `.task/TASK-070..073.json`, `context.md`, `roadmap.md`, `state.json`, and `index.json`.
- Result: confirmed the follow-up boundaries cover pipeline preprocessing, internal tool provider sources, ChatUI project/session ownership, and provider-neutral TTS streaming audio queues.
- Adjustment: no Rust source changes; immediate pointer remains `M7-R26-knowledge-base-boundary` / `TASK-031`.
- Verification: validate workflow JSON and task-count consistency after this sync.

## Round 40 - TASK-031 Knowledge Base Boundary

- Scope: new `astrbot-kb` crate and workspace registration.
- Strategy: follow AstrBot's KB separation for documents, parser, chunking, retrieval, rank fusion, embedding, rerank, vector store, and prompt formatting while keeping provider capabilities and runtime wiring outside the KB crate.
- Result: added typed KB IDs/chunks, profile/document/media DTOs, parser and chunker ports, embedding orchestration, vector-store and sparse retrieval ports, reciprocal rank fusion, hybrid retrieval with optional rerank, retrieval context formatter, and focused tests.
- AstrBot reference: `knowledge_base/kb_mgr.py`, `kb_helper.py`, `models.py`, `chunking/*`, `parsers/base.py`, and `retrieval/{manager,rank_fusion,sparse_retriever}.py`.
- Adjustment: next implementation pointer moves to `M7-R27-persistence-migration-boundary` / `TASK-032`.
- Verification: `cargo fmt --all`, `cargo test -p astrbot-kb`, `cargo test -p astrbot-provider`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 41 - TASK-032 Persistence Migration Boundary

- Scope: `astrbot-storage` schema, migration, stats, backup, repository descriptor, and SQLite planning boundaries.
- Strategy: preserve existing storage repository ports while extracting AstrBot-inspired DB schema, migration markers, platform stats, backup manifest/import/export, and SQLite PRAGMA planning into typed modules without wiring a concrete database backend.
- Result: added typed main DB schema descriptions, migration runner/state, platform stats repository, backup manifest/export/import ports, repository backend descriptors, and SQLite plan/create-table helpers.
- AstrBot reference: `core/db/__init__.py`, `core/db/sqlite.py`, `core/db/po.py`, `core/db/migration/helper.py`, `core/db/migration/migra_token_usage.py`, `core/backup/constants.py`, `core/backup/exporter.py`, and `core/backup/importer.py`.
- Adjustment: next implementation pointer moves to `M7-R28-platform-webhook-security-boundary` / `TASK-033`.
- Verification: `cargo fmt --all`, `cargo test -p astrbot-storage`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 42 - Additional Decoupling Audit

- Scope: current Rust hotspots after `TASK-031` and `TASK-032`, plus AstrBot KB manager, DB schema, MCP protocol/client, and agent hook/run-context references.
- Strategy: avoid Rust source changes; record only boundary candidates not already covered precisely by `TASK-033` through `TASK-073`.
- Result: recorded `TASK-074` through `TASK-077` for storage schema table-family split, MCP wire primitive/protocol type split, KB ingestion/indexing job boundaries, and agent hook/run-context/response/tool-image-cache boundaries.
- AstrBot reference: `core/db/{po.py,sqlite.py,migration/*}`, `core/agent/{mcp_client.py,mcp_stdio_client.py,mcp_subcapability_bridge.py,hooks.py,run_context.py,message.py,response.py,tool_image_cache.py}`, and `core/knowledge_base/{kb_mgr.py,kb_db_sqlite.py,kb_helper.py}`.
- Adjustment: these are follow-up guardrails. The immediate implementation pointer remains `M7-R28-platform-webhook-security-boundary` / `TASK-033`.
- Verification: workflow records only; JSON validity and task-count consistency should be checked after writes.

## Round 43 - TASK-033 Platform Webhook Security Boundary

- Scope: new shared platform common modules for webhook callback security, callback server contracts, long-connection lifecycle, and callback queues.
- Strategy: follow AstrBot QQ Official webhook and WeCom AI Bot separation: callback verification/server/queue/long-connection stay in transport-level boundaries, while adapter event/message conversion remains adapter-local.
- Result: added `security.rs`, `webhook.rs`, `long_connection.rs`, and `queue.rs` under `astrbot-platform/src/adapters/common`; exported canonical common names through `adapters/common`, `adapters`, and crate root; added `LongConnection` to `PlatformTransportKind`.
- AstrBot reference: `qqofficial_webhook/qo_webhook_server.py`, `qqofficial_webhook/qo_webhook_adapter.py`, `wecom_ai_bot/wecomai_server.py`, `wecom_ai_bot/WXBizJsonMsgCrypt.py`, `wecom_ai_bot/wecomai_long_connection.py`, and `wecom_ai_bot/wecomai_queue_mgr.py`.
- Adjustment: next implementation pointer moves to `M7-R29-provider-test-media-fixture-boundary` / `TASK-034`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-platform`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` passed.

## Round 44 - TASK-034 Provider Test Media Fixture Boundary

- Scope: provider integration test support for temporary audio inputs, TTS output directories, and generated audio cleanup.
- Strategy: follow AstrBot's central temp media path idea from TTS providers, but keep this pass test-only so provider behavior and assertions remain unchanged.
- Result: added `tests/support/media_fixture.rs` with `TempAudioFile`, `TempOutputDir`, and `GeneratedAudioFile`; migrated OpenAI/Xinference STT, audio media conversion, TTS adapter tests, and provider registry speech/TTS tests away from repeated timestamp path helpers and manual cleanup.
- AstrBot reference: `provider/sources/openai_tts_api_source.py`, `gemini_tts_source.py`, `minimax_tts_api_source.py`, `volcengine_tts.py`, `gsvi_tts_source.py`, and `core/utils/astrbot_path.py`.
- Adjustment: next implementation pointer moves to `M7-R30-provider-media-artifact-boundary` / `TASK-035`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-provider`, `cargo clippy -p astrbot-provider -- -D warnings`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` passed.

## Round 45 - TASK-035 Provider Media Artifact Boundary

- Scope: shared generated media artifact writer for file-based TTS provider outputs.
- Strategy: follow AstrBot's shared temp path policy while keeping provider-specific protocol parsing, audio decoding, and request IDs inside concrete adapters.
- Result: added `crates/astrbot-provider/src/media/{mod.rs,artifact.rs}` with `GeneratedMediaArtifactWriter`; migrated OpenAI, Gemini, MiniMax, Volcengine, and GSVI TTS providers away from repeated output directory, filename, `create_dir_all`, `fs::write`, empty-audio, and display-path helpers.
- Additional scan: no new independent `TASK-078` is needed. Existing backlog already covers the remaining hotspots: streaming strategy, multimodal preparation, path/temp cleanup, non-chat provider protocol DTOs, provider manager buckets, TTS factory mapping, storage schema catalog, MCP wire primitives, and KB ingestion/index jobs.
- Maestro state adjustment: normalized stale plan flags so `TASK-033` and `TASK-034` match their completed task files, while `TASK-011` and `TASK-058` remain pending. Completed count remains 34 after adding `TASK-035`.
- AstrBot reference: `provider/provider.py`, `provider/sources/openai_tts_api_source.py`, `gemini_tts_source.py`, `minimax_tts_api_source.py`, `volcengine_tts.py`, `gsvi_tts_source.py`, and `core/utils/astrbot_path.py`.
- Adjustment: next implementation pointer moves to `M7-R31-streaming-strategy-boundary` / `TASK-036`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-provider`, `cargo clippy -p astrbot-provider -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 46 - TASK-036 Streaming Strategy Boundary

- Scope: provider streaming parser helpers, streamed text delta normalization, and unsupported streaming policy in `astrbot-provider`.
- Strategy: follow AstrBot's streaming capability/config split while keeping Rust protocol DTO parsing in provider protocol modules and keeping core/pipeline delivery transport-neutral.
- Result: added `streaming/sse.rs`, `streaming/chunk.rs`, and `streaming/policy.rs`; OpenAI-compatible chat and MiniMax TTS now share SSE extraction, OpenAI-compatible chat shares chunk normalization, and Anthropic/Gemini share unsupported streaming rejection.
- Additional scan: no new independent `TASK-078` is needed. Existing backlog already covers the remaining decoupling spaces that would otherwise absorb streaming growth: multimodal preparation, live-agent feedback orchestration, and TTS streaming audio queues.
- AstrBot reference: `core/config/default.py`, `core/provider/provider.py`, and `core/agent/runners/tool_loop_agent_runner.py`.
- Adjustment: next implementation pointer moves to `M7-R32-multimodal-preparation-boundary` / `TASK-037`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-provider`, `cargo clippy -p astrbot-provider -- -D warnings`, `cargo test -p astrbot-pipeline`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 47 - TASK-037 Multimodal Preparation Boundary

- Scope: new `astrbot-agent/src/multimodal/*` modules and `astrbot-agent` facade exports.
- Strategy: follow AstrBot main-agent's split between image captioning, quoted image fallback, modality checks, and provider request preparation while keeping Rust provider adapters limited to protocol serialization/parsing.
- Result: added image caption request decorator and captioner trait, provider-backed image captioner adapter, quoted image attachment policy, and provider modality filter decorator for unsupported image/tool-use behavior.
- AstrBot reference: `astr_main_agent.py` `_request_img_caption`, `_ensure_img_caption`, `_append_quoted_image_attachment`, `_modalities_fix`, and `_sanitize_context_by_modalities`, plus `quoted_message/image_refs.py` and `provider/entities.py`.
- Adjustment: next implementation pointer moves to `M7-R33-path-temp-artifact-boundary` / `TASK-038`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-agent`, `cargo clippy -p astrbot-agent -- -D warnings`, `cargo test -p astrbot-pipeline`, `cargo test -p astrbot-provider`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 48 - TASK-038 Path And Temp Artifact Boundary

- Scope: runtime path configuration, storage temp artifact lifecycle, and provider generated-media default output path.
- Strategy: follow AstrBot's centralized `astrbot_path.py` and `TempDirCleaner`, but keep the first Rust pass as typed config and cleanup policy boundaries rather than wiring a background cleaner.
- Result: added `RuntimePathConfig`/`RuntimePathLayout`, `RuntimeConfig.paths`, config schema/UI metadata entries, `TempArtifactRoot`, safe temp artifact descriptors, cleanup policy/plans, and `TempArtifactCleaner`; provider TTS default output now derives from `data/temp/generated_media/tts`.
- AstrBot reference: `core/utils/astrbot_path.py`, `core/utils/temp_dir_cleaner.py`, `core/backup/exporter.py`, and `core/backup/importer.py`.
- Adjustment: next implementation pointer moves to `M7-R34-t2i-render-boundary` / `TASK-039`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-storage`, `cargo test -p astrbot-runtime`, `cargo test -p astrbot-provider`, `cargo clippy -p astrbot-storage -- -D warnings`, `cargo clippy -p astrbot-runtime -- -D warnings`, `cargo clippy -p astrbot-provider -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 49 - Additional Decoupling Audit

- Scope: current Rust hotspots after `TASK-038`, plus AstrBot provider response entities, rich platform identity, dashboard session management, provider media input handling, and update/maintenance routes.
- Strategy: avoid Rust source changes; record only boundary candidates not already covered by `TASK-039` through `TASK-077`.
- Result: recorded `TASK-078` through `TASK-082` for provider response metadata/token usage/reasoning/tool-call payloads, platform identity/member/group permission resolution, scoped session rules and batch preferences, media normalization/data URL/download services, and maintenance update/migration/package-install operation boundaries.
- AstrBot reference: `core/provider/entities.py`, provider sources for OpenAI/Gemini/Anthropic, `core/platform/{astrbot_message.py,astr_message_event.py,message_session.py}`, `dashboard/routes/session_management.py`, `dashboard/routes/chat.py`, `dashboard/routes/update.py`, `core/utils/io.py`, `core/star/updator.py`, and DB migration helper code.
- Adjustment: immediate implementation pointer remains `M7-R34-t2i-render-boundary` / `TASK-039`; the new tasks are guardrails for later provider response, platform permission, dashboard session, media, and maintenance parity work.
- Verification: workflow records only; JSON validity and task-count consistency should be checked after writes.

## Round 50 - TASK-039 T2I Render Boundary

- Scope: new `astrbot-render` crate and workspace registration.
- Strategy: follow AstrBot's network/local renderer and user-overrides-builtin template manager, but use Rust traits and typed DTOs so concrete rasterization/API clients remain behind a renderer boundary.
- Result: added `T2iRenderer`, `T2iRenderRequest`, `T2iRenderResult`, `RenderStrategy`, `RenderMode`, `RenderFormat`, `RenderArtifact`, `TemplateName`, `TemplateCatalog`, and `TemplateRenderer`. RespondStage and WebChat routes were not touched.
- AstrBot reference: `core/utils/t2i/renderer.py`, `core/utils/t2i/network_strategy.py`, `core/utils/t2i/local_strategy.py`, and `core/utils/t2i/template_manager.py`.
- Adjustment: next implementation pointer moves to `M7-R35-observability-trace-boundary` / `TASK-040`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-render`, `cargo clippy -p astrbot-render -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 51 - Additional Decoupling Audit

- Scope: current Rust workspace after `TASK-039`, plus AstrBot T2I, shared IO/download, KB dashboard, metrics/stat, and tool reference extraction references.
- Strategy: avoid source changes; record only boundary candidates not already covered by `TASK-040` through `TASK-082`.
- Result: recorded `TASK-083` through `TASK-087` for concrete T2I network/local/markdown/font strategies, shared network download/TLS/progress/cache, KB management/preflight/upload tasks, metrics/usage accounting, and route-independent tool references.
- AstrBot reference: `core/utils/t2i/*`, `core/utils/io.py`, `core/utils/http_ssl.py`, `dashboard/routes/knowledge_base.py`, `core/utils/metrics.py`, `dashboard/routes/stat.py`, `dashboard/routes/{chat,live_chat,open_api}.py`, and `builtin_stars/web_searcher/main.py`.
- Adjustment: immediate implementation pointer remains `M7-R35-observability-trace-boundary` / `TASK-040`; the new tasks are guardrails for later render, network IO, KB dashboard, metrics, and citation parity work.
- Verification: workflow records only; JSON validity and task-count consistency should be checked after writes.

## Round 52 - TASK-040 Observability Trace Boundary

- Scope: new `astrbot-observability` crate plus light lifecycle-sink wiring in runtime, provider, and platform managers.
- Strategy: keep the new boundary narrow and typed, preserve stop/restart behavior, and avoid dashboard/WebChat coupling while exposing in-memory test sinks for observability assertions.
- Result: added typed status events, log buffer, and trace span/event boundaries; runtime/provider/platform managers now emit lifecycle status through a trait sink; runtime restart preserves the configured sink.
- AstrBot reference: `core/utils/trace.py`, `core/platform/manager.py`, and `core/provider/manager.py`.
- Adjustment: next implementation pointer moves to `M7-R36-event-bus-routing-boundary` / `TASK-041`.
- Verification: `cargo fmt --all --check`, `cargo test -p astrbot-observability`, `cargo test -p astrbot-runtime`, `cargo test -p astrbot-provider`, `cargo test -p astrbot-platform`, `cargo clippy -p astrbot-observability -- -D warnings`, `cargo clippy -p astrbot-runtime -- -D warnings`, `cargo clippy -p astrbot-provider -- -D warnings`, `cargo clippy -p astrbot-platform -- -D warnings`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` passed.

## Round 53 - Additional Decoupling Audit

- Scope: current Rust workspace after `TASK-040`, with focus on platform core contracts, agent request composition, MCP/tool/agent tests, and AstrBot platform/agent/MCP references.
- Strategy: avoid Rust source changes; record only boundaries not already covered by `TASK-041` through `TASK-087`.
- Result: recorded `TASK-088` through `TASK-090` for platform core contract/config/sink/validation split, agent request envelope/decorator/port composition split, and agent/MCP/tool behavior-test splits.
- AstrBot reference: `core/platform/{platform.py,platform_metadata.py,register.py,manager.py}`, `astr_main_agent.py`, `pipeline/process_stage/method/agent_request.py`, `agent/context/manager.py`, `agent/mcp_client.py`, and `provider/func_tool_manager.py`.
- Adjustment: immediate implementation pointer remains `M7-R36-event-bus-routing-boundary` / `TASK-041`; the new items are follow-up guardrails after event routing.
- Verification: workflow records only; JSON validity and task-count consistency should be checked after writes.
