# astrbot-rs Pipeline-First Roadmap

## Goal

Port AstrBot to Rust by stabilizing the lower-level event and pipeline kernel first, then moving upward through policy stages, process/provider orchestration, response delivery, runtime reload, and finally platform/dashboard breadth.

## Current Baseline

- `astrbot-core` has typed `MessageEvent`, `MessageChain`, thin `EventBus`, and message components for plain/image/reply/record/video/file/mention.
- `astrbot-pipeline` has an ordered scheduler plus wake, whitelist, session status, rate limit, content safety, process, and respond stages; `PluginStage` and `ProviderStage` remain as compatibility building blocks.
- `astrbot-provider` has registry/manager and OpenAI-compatible text/image request support.
- `astrbot-plugin` has a minimal Star-like handler registry and plugin stage.
- `astrbot-platform` now keeps core traits, registry, manager, and concrete adapters in separate modules; `astrbot-runtime`, `astrbot-web`, and `astrbot-cli` prove Platform -> EventBus -> Pipeline -> Provider -> Respond with mock/console/webchat paths.
- Recent WebChat work should be treated as boundary validation, not the primary migration direction.

## Migration Principles

- Bottom-up: event and pipeline contracts before platform/dashboard breadth.
- Decoupled: policy, process, provider, response decoration, and transport stay in separate crates/stages.
- Evidence-based: each milestone references AstrBot files and requires Rust tests that prove behavior.
- Compatibility without copying: retain AstrBot concepts, but express them as Rust traits, typed config, and registries.

## Milestones

### M1 Pipeline Kernel

Status: completed

Reference:
- `astrbot/core/pipeline/scheduler.py`
- `astrbot/core/pipeline/stage.py`
- `astrbot/core/pipeline/stage_order.py`

Deliverables:
- Stage registry with deterministic order equivalent to AstrBot `STAGES_ORDER`.
- Stage metadata and initialization hook so runtime can build default pipelines without hardcoding each stage inline.
- Scheduler semantics for `Continue` / `Stop`, event stop state, and stage lifecycle.
- Explicit test matrix for stage order, stop behavior, and empty/default pipeline behavior.

Next task:
- `M3-T1-unified-process-stage`

Completed:
- `M1-T1-stage-registry-and-order`: built `PipelineStageRegistry`, deterministic built-in order, and runtime default scheduler construction through registry.
- `M1-T2-stage-initialize-hook`: added `PipelineStage::initialize`, scheduler stage initialization, and runtime default pipeline initialization.
- `M1-T3-default-pipeline-builder`: added `DefaultPipelineBuilder`, runtime delegation, and scheduler stop-control coverage.

Success criteria:
- Runtime can construct the default pipeline from registered stage factories.
- Tests prove registered stages execute in configured order and stop correctly.
- Existing plugin/provider/respond tests still pass.

### M2 Pipeline Policy Stages

Depends on: M1

Status: completed

Reference:
- `waking_check/stage.py`
- `whitelist_check/stage.py`
- `session_status_check/stage.py`
- `rate_limit_check/stage.py`
- `content_safety_check/stage.py`

Deliverables:
- Wake/mention gate as a policy stage.
- Whitelist/session enabled/rate limit stages backed by typed config ports.
- Content safety strategy trait with a keyword strategy first.
- Tests for allow/deny/stop behavior without requiring real platforms/providers.

Completed:
- `M2-T1-wake-check-stage`: added `WakeCheckStage`, `WakeCheckConfig`, direct/group session metadata, mention components, and tests for prefix/mention/deny behavior.
- `M2-T2-whitelist-session-rate-limit-ports`: added typed whitelist config, session status port, fixed-window rate limit config/stage, runtime policy wiring, and isolated allow/deny/limit tests.
- `M2-T3-content-safety-strategy`: added content safety strategy trait, keyword strategy, content safety stage, runtime config wiring, and allow/block tests.

Success criteria:
- Non-eligible events stop before process/provider stages.
- Policy stages are configured through `PipelineContext`, not runtime globals.

### M3 Process Stage

Depends on: M1, M2

Status: completed

Reference:
- `process_stage/stage.py`
- `process_stage/method/star_request.py`
- `process_stage/method/agent_request.py`
- `astr_main_agent.py`

Deliverables:
- A unified `ProcessStage` coordinating plugin handlers and provider fallback.
- Provider request model with selected provider, wake prefix, multimodal inputs, and tool placeholders.
- Provider fallback/error handling boundary.

Completed:
- `M3-T1-unified-process-stage`: added `ProcessStage`, changed the default built-in order to `wake -> whitelist -> session_status -> rate_limit -> content_safety -> process -> respond`, and preserved `PluginStage`/`ProviderStage` for focused tests and custom schedulers.
- `M3-T2-provider-request-model`: added event-level `ProviderRequest`, `MessageEvent` request storage, conversion into `ChatRequest`, and tests proving plugin-generated LLM requests flow through `ProcessStage`.
- `M3-T3-provider-fallback-policy`: added `ProviderFallbackConfig`, runtime wiring, wake gating, disabled-provider fallback behavior, and generic provider-error replies.

Success criteria:
- Plugin command result suppresses provider fallback.
- Plugin-generated provider requests flow into provider execution.
- Provider-disabled config prevents LLM calls.

### M4 Respond And Result Decoration

Depends on: M3

Status: completed

Reference:
- `result_decorate/stage.py`
- `respond/stage.py`

Deliverables:
- Result decoration stage for prefixes and output transforms.
- Respond validators for empty chains and supported component types.
- Segmented reply and streaming response boundaries.

Completed:
- `M4-T1-result-decorate-stage`: added `ResultDecorateStage` after `process`, runtime config wiring, and reply-prefix decoration for the first plain result component.
- `M4-T2-respond-validators`: added send-chain validators that remove empty plain/media components, skip reply/mention-only results, preserve stoppage control, and keep `RespondStage` focused on final delivery.
- `M4-T3-streaming-response-boundary`: added a typed `MessageStream` boundary, `MessageSink::send_streaming`, streaming finish duplicate suppression, and tests for streaming vs non-streaming response paths.

Success criteria:
- Empty result chains are skipped.
- Streaming and non-streaming response paths are independently testable.
- WebChat/console reply sinks remain transport-only.

### M5 Context, Session, And Memory

Depends on: M3

Status: completed

Reference:
- `context.py`
- `context_utils.py`
- `astr_main_agent.py`

Deliverables:
- Conversation/session state interfaces.
- Provider preference per session.
- Quote/reply context extraction policy.
- Context compression placeholder boundary.

Completed:
- `M5-T1-session-context-ports`: added `SessionContextPort`/`EmptySessionContextPort`, injected context messages into provider requests, and covered `ProcessStage` context flow with tests.
- `M5-T2-provider-preference-storage`: added provider preference ports, in-memory session preference storage, provider manager request routing, and tests for session-selected provider IDs.
- `M5-T3-quote-context-policy`: added quote context policy ports, injected reply `selected_text` into provider request user content parts, and kept reply-only messages from triggering provider fallback.

Success criteria:
- ProviderStage/ProcessStage can read session context through traits, not concrete storage.
- Tests prove per-session provider selection and context injection.

### M6 Runtime Assembly And Reload

Depends on: M1-M5

Status: completed

Reference:
- `core_lifecycle.py`
- `initial_loader.py`

Deliverables:
- Runtime assembles managers, registries, policy config, and default pipeline through a single lifecycle.
- Reload/restart preserves or discards state according to explicit policy.
- Termination hooks for provider/plugin/platform managers.

Completed:
- `M6-T1-runtime-default-pipeline-assembly`: runtime now owns and injects provider preference state into the default `PipelineContext`, while continuing to build the stage chain through `DefaultPipelineBuilder`.
- `M6-T2-reload-state-policy`: added explicit restart state policy; provider preference is preserved by default and can be discarded through config.
- `M6-T3-manager-termination-hooks`: added terminate hooks for provider, platform, and plugin managers, and wired runtime stop through those lifecycle boundaries.

Success criteria:
- Runtime restart rebuilds pipeline from config.
- Reload tests prove old background tasks are stopped.

### M7 Platform, Provider, And Dashboard Breadth

Depends on: M1-M6

Status: in_progress

Reference:
- `platform/sources/*`
- `provider/sources/*`
- `dashboard`

Deliverables:
- Real platform adapters behind `PlatformRegistry`.
- Provider parity prioritized by OpenAI-compatible, then high-value providers.
- WebChat attachment storage, SSE/WebSocket return path, auth/session management.
- Dashboard API/UI only after runtime and pipeline contracts stabilize.

Completed:
- `M7-T1-real-platform-adapters`: added a minimal OneBot/Aiocqhttp-inspired adapter behind `PlatformRegistry`, with private/group text normalization into unified `MessageEvent`, typed direct/group session kind, runtime wiring, and focused tests. Native OneBot network transport remains deferred.
- `M7-T2a-openai-compatible-provider-aliases`: registered AstrBot-compatible OpenAI-family provider types (`zhipu`, `groq`, `xai`, `aihubmix`, `openrouter`) through `ProviderRegistry`, reusing the typed OpenAI-compatible implementation and preserving OpenRouter/AIHubMix default headers.
- `M7-T2b-anthropic-chat-provider`: added native `anthropic_chat_completion` behind `ProviderRegistry`, including Messages API request mapping for text/system/context, data URL images, response/error parsing, runtime config mapping, and focused tests.
- `M7-T2c-gemini-chat-provider`: added native `googlegenai_chat_completion` behind `ProviderRegistry`, including Google GenAI generateContent request mapping for text/system/context, data URL images, safety/policy finish reason errors, response/error parsing, runtime config mapping, and focused tests.
- `M7-T2d-provider-capability-boundary`: added AstrBot-inspired provider capability metadata (`chat_completion`, `speech_to_text`, `text_to_speech`, `embedding`, `rerank`) to `ProviderRegistry`, while keeping `ProviderManager` chat-only and preventing non-chat metadata from being built as chat providers.
- `M7-T2e-embedding-provider-boundary`: added provider-crate-only embedding trait/request/response/mock provider plus registry factory and manager routing, mirroring AstrBot's separate embedding capability bucket while deferring concrete OpenAI/Gemini embedding HTTP providers.
- `M7-T2f-openai-embedding-provider`: added AstrBot-compatible `openai_embedding` as a concrete `EmbeddingProvider`, including `/embeddings` request serialization, default model/dimensions/base URL normalization, response/error parsing, registry construction, and focused tests.
- `M7-T2g-gemini-embedding-provider`: added AstrBot-compatible `gemini_embedding` as a concrete `EmbeddingProvider`, including Google REST `embedContent`/`batchEmbedContents` request mapping, response/error parsing, registry construction, and focused tests.
- `M7-T2h-rerank-provider-boundary`: added provider-crate-only rerank request/response/trait/mock provider plus registry factory and manager routing, mirroring AstrBot's separate rerank capability bucket while deferring concrete VLLM/Xinference/Bailian adapters.
- `M7-T2i-vllm-rerank-provider`: added AstrBot-compatible `vllm_rerank` as the first concrete `RerankProvider`, including `/v1/rerank` request serialization, bearer auth, response/error parsing, registry construction, and focused tests.
- `M7-T2j-bailian-rerank-provider`: added AstrBot-compatible `bailian_rerank` as a concrete `RerankProvider`, including DashScope text-rerank payload mapping, required bearer auth, `qwen3-rerank` instruct handling, API-code error parsing, registry construction, and focused tests.
- `M7-T2k-stt-tts-provider-boundary`: added provider-crate-only STT/TTS request/response/trait/mock providers plus registry factories and manager routing, mirroring AstrBot's separate `speech_to_text` and `text_to_speech` capability buckets while deferring concrete voice providers.
- `M7-T2l-xinference-rerank-provider`: added AstrBot-compatible `xinference_rerank` as a concrete `RerankProvider`, including running-model UID resolution, optional model launch, `/v1/rerank` request serialization, registry construction, and focused tests.
- `M7-T2m-openai-tts-provider`: added AstrBot-compatible `openai_tts_api` as the first concrete `TextToSpeechProvider`, including OpenAI-compatible `/audio/speech` request serialization, bearer auth, generated audio file output, registry construction, and focused tests.
- `M7-T2n-openai-stt-provider`: added AstrBot-compatible `openai_whisper_api` as the first concrete `SpeechToTextProvider`, including OpenAI-compatible `/audio/transcriptions` multipart requests, bearer auth, local/HTTP audio input, auth-isolated audio downloads, registry construction, and focused tests. SILK/AMR/Tencent audio conversion remains deferred until a media conversion boundary exists.
- `M7-T2o-xinference-stt-provider`: added AstrBot-compatible `xinference_stt` as a concrete `SpeechToTextProvider`, including lazy Xinference audio model UID resolution, optional model launch, OpenAI-compatible `/v1/audio/transcriptions` multipart requests, local/HTTP audio input, auth-isolated audio downloads, registry construction, and focused tests. SILK/AMR/Tencent audio conversion remains explicitly deferred.
- `M7-T2p-media-conversion-boundary`: added a shared provider-crate audio input and media conversion boundary (`AudioInputLoader`, `AudioMediaConverter`, conversion detection), then refactored OpenAI Whisper STT and Xinference STT to use it. Default conversion is explicit unsupported behavior until a concrete ffmpeg/Tencent adapter is designed.
- `M7-T2q-gemini-tts-provider`: added AstrBot-compatible `gemini_tts` as a concrete `TextToSpeechProvider`, including Gemini REST `generateContent` audio request mapping, voice config, inline base64 PCM decoding, local 24 kHz mono 16-bit WAV output, registry construction, and focused tests.
- `M7-T2r-non-chat-provider-runtime-config`: extended runtime/provider manager assembly so STT, TTS, Embedding, and Rerank configs enter separate capability buckets from typed runtime config, while the pipeline still only receives chat providers.
- `M7-T2s-volcengine-tts-provider`: added AstrBot-compatible `volcengine_tts` as a concrete `TextToSpeechProvider`, preserving Volcengine app/audio/request payload fields, `Bearer;` auth header, base64 MP3 response decoding, registry construction, runtime option mapping, and focused tests.
- `M7-T2t-minimax-tts-provider`: added AstrBot-compatible `minimax_tts_api` as a concrete `TextToSpeechProvider`, preserving MiniMax streaming request payload, `GroupId` query, bearer auth, SSE hex MP3 chunk collection, timber_weights mode, registry construction, runtime option mapping, and focused tests.
- `M7-T2u-gsvi-tts-provider`: added AstrBot-compatible `gsvi_tts_api` as a concrete `TextToSpeechProvider`, preserving GSVI `/tts` GET query shape, optional character/emotion parameters, local WAV output, registry construction, runtime option mapping, and focused tests.
- `M7-R1-platform-module-split`: paused new parity work and split `astrbot-platform` into `core`, `built`, `registry`, `manager`, and `adapters/*`, so future WeChat/QQ/OneBot transport work has a dedicated adapter namespace while crate root remains a public re-export surface.
- `M7-R2-decoupling-audit`: recorded the next structural refactor backlog in `.workflow/scratch/refactor-decoupling-audit-2026-05-16`, identifying `astrbot-runtime`, `astrbot-provider`, `astrbot-web`, `astrbot-core::message`, and `astrbot-pipeline::context` as the next decoupling targets before more parity migration.
- `M7-R3a-runtime-module-split`: split `astrbot-runtime/src/lib.rs` into facade re-exports plus focused `config`, `provider_config`, `policy_config`, `platform_config`, `handle`, `assembly`, `ports`, `defaults`, `config_io`, and `tests` modules without behavior changes.
- `M7-R3b-provider-registry-decoupling`: split `astrbot-provider/src/registry.rs` into `constants`, `capability`, `config`, `factories`, `manager`, and focused `registry` modules while preserving crate root public exports and provider behavior.
- `M7-R3c-provider-public-trait-test-decoupling`: split provider public traits/request/response/mock providers into `chat`, `speech`, `tts`, `embedding`, `rerank`, and `mock` modules, and split `provider_registry` integration tests by capability.
- `M7-R4a-web-http-boundary-decoupling`: split `astrbot-web/src/lib.rs` into facade re-exports plus `dto`, `message_parts`, `error`, `routes`, `server`, and `tests` modules while keeping WebChat HTTP behavior unchanged.
- `M7-R4b-core-message-domain-decoupling`: split `astrbot-core::message` into component, chain, session, sink, result/stream, provider request/tool DTO, event, and test modules while preserving public re-exports.
- `M7-R4c-pipeline-context-decoupling`: split `astrbot-pipeline::context` into context composition, policy config, session ports, provider preference, quote context, content safety, and result decoration modules while preserving public re-exports.
- `M7-R5-plugin-sdk-sandbox-design`: split `astrbot-plugin` into event, handler, registry, filter, manifest, sandbox, SDK, and test modules, preserving current registry behavior while adding typed plugin permissions, tool capabilities, sandbox profiles, manifest declarations, and SDK context/test harness surfaces.
- `M7-R6a-runtime-provider-config-test-decoupling`: split runtime provider config mappings by chat, STT, TTS, embedding, and rerank capability, and split runtime tests into message loop, provider config, policy, platform, lifecycle, and config IO modules.
- `M7-R6c-additional-decoupling-audit`: compared current Rust hotspots with AstrBot `PlatformManager`, `ProviderManager`, `StarManager`, tool executor, and config defaults; recorded `TASK-016` through `TASK-020` for provider test support, runtime task supervision, plugin loader/lifecycle, tool execution/sandbox, and config schema boundaries.
- `M7-R6b-provider-config-factory-manager-decoupling`: split provider config types, concrete provider factories, manager config set, lifecycle termination, and per-capability routing into focused modules while preserving `astrbot_provider` facade exports and registry/manager behavior.
- `M7-R7-platform-webchat-media-boundaries`: split OneBot/WebChat adapters into adapter-local event/message modules and added WebChat attachment/history service boundaries while keeping platform registry/manager and HTTP behavior unchanged.
- `M7-R8-pipeline-registry-boundary`: split pipeline stage order constants, built-in registration, registry entries, and registry tests into focused modules while preserving `DefaultPipelineBuilder` and scheduler behavior.
- `M7-R9-provider-http-helper-boundary`: extracted shared provider HTTP helpers for base URL joining, client/header construction, bearer/API-key auth, custom headers, and common JSON error extraction while keeping provider-specific payload mapping inside concrete adapters.
- `M7-R10-cli-entrypoint-boundary`: split CLI argument parsing, init/run/smoke command handlers, WebChat server launcher, and CLI tests into focused modules while keeping `main.rs` as a thin async entrypoint.
- `M7-R11-provider-test-support-boundary`: completed shared provider test HTTP fixtures, response sequencing, request capture, and header assertion helpers so adapter parity tests no longer carry socket-level harness code.
- `M7-R12-runtime-task-supervision-boundary`: split runtime handle into facade/runtime/supervisor/restart/testing modules, isolating background task lifecycle and restart state transfer while preserving public `AstrbotRuntime`/`RuntimeHandle` behavior.
- `M7-R13-plugin-loader-lifecycle-boundary`: added typed plugin loader, metadata, dependency, lifecycle, hot-reload, platform-extension, and web API descriptor boundaries while keeping dynamic loading deferred.
- `M7-R14-tool-execution-sandbox-boundary`: added typed plugin tool declarations, handoff/background descriptors, executor traits, sandboxed execution wrapper, and capability decisions while keeping real local/MCP/handoff/background execution deferred.
- `M7-R15-config-schema-boundary`: split runtime config defaults, env loading, secret redaction, migration/default-merge planning, schema fields, and dashboard UI metadata into typed config submodules.
- `M7-R16-web-test-boundary`: split WebChat HTTP tests into support, submit, message-part, history, and live-server modules while preserving route behavior.
- `M7-R17-provider-protocol-boundary`: extracted OpenAI-compatible, Gemini, Anthropic, MiniMax TTS, and Xinference protocol DTOs/parsers into `astrbot-provider/src/protocol` while keeping concrete adapters as config/HTTP orchestration layers.
- `M7-R18-platform-transport-boundary`: added shared platform transport/media boundary types and OneBot session/transport modules while keeping `PlatformRegistry`/`PlatformManager` as runtime entry points.
- `M7-R19-storage-boundary`: introduced `astrbot-storage` repository ports for conversation history, attachments, provider preferences, and config snapshots; WebChat history and provider preference now flow through storage traits with in-memory implementations.
- `M7-R20-agent-runner-boundary`: introduced `astrbot-agent` with typed `AgentRunner`, `ChatAgentRunner`, fallback policy, request decorators, persona prompt decorator, and tool-loop policy placeholders; pipeline provider fallback now delegates through this agent boundary.
- `M7-R21-management-api-boundary`: separated dashboard-facing management APIs into `astrbot-web::management` for status, provider, platform, and plugin snapshots while WebChat submit/history routes remain transport-specific.
- `M7-R22-provider-registry-builtin-boundary`: split provider built-in registration, factory trait object aliases, metadata lookup, and registry error helpers into focused `registry/*` submodules while keeping `ProviderRegistry` as the public entry point.
- `M7-R23-agent-context-boundary`: introduced `astrbot-agent::context` for context windows, token budget/counters, truncation policy, compression trait, and request decorator integration while keeping pipeline/provider code free of context compression logic.
- `M7-R24-mcp-boundary`: added `astrbot-mcp` with typed MCP config, client lifecycle, tool/resource/prompt bridges, sampling, elicitation, and roots allowlist boundaries before real MCP runtime wiring.
- `M7-R25-tool-schema-command-boundary`: added `astrbot-tool` for tool catalog descriptors, provider-format schema serializers, activation policy, command descriptors, and conflict detection outside provider/plugin registry internals.
- `M7-R26-knowledge-base-boundary`: added `astrbot-kb` for KB documents, parsers, chunking, embedding orchestration, vector-store ports, sparse retrieval, rank fusion, hybrid retrieval, rerank integration, and context formatting while provider embedding/rerank capabilities remain in `astrbot-provider`.
- `M7-R27-persistence-migration-boundary`: extended `astrbot-storage` with typed main DB schema descriptions, migration runner/state, platform stats repository, backup manifest/export/import ports, repository backend descriptors, and SQLite planning/pragmas.
- `M7-R27b-post-persistence-decoupling-audit`: recorded `TASK-074` through `TASK-077` for storage schema catalog modules, MCP wire primitives, KB ingestion/indexing jobs, and agent hook/run-context side-channel boundaries.
- `M7-R28-platform-webhook-security-boundary`: added shared platform webhook security, callback server, long-connection, and callback queue boundaries under `astrbot-platform::adapters::common`, keeping adapter event conversion and `PlatformManager` separate.
- `M7-R29-provider-test-media-fixture-boundary`: added provider test media fixtures for temporary audio input files, TTS output directories, and generated audio cleanup so voice/media tests stay behavior-focused.
- `M7-R30-provider-media-artifact-boundary`: added a shared provider media artifact writer for TTS output directory, safe extension, filename, write, and display-path policy across OpenAI, Gemini, MiniMax, Volcengine, and GSVI TTS adapters.
- `M7-R31-streaming-strategy-boundary`: split provider streaming helpers into generic SSE extraction, streamed text delta normalization, and unsupported streaming policy while keeping protocol parsing in provider modules and `RespondStage` delivery-only.
- `M7-R32-multimodal-preparation-boundary`: added `astrbot-agent::multimodal` for image caption request decoration, quoted image attachment fallback, provider modality filtering, and context image/tool sanitization while keeping `ProcessStage` and provider adapters focused.
- `M7-R33-path-temp-artifact-boundary`: added runtime path config and storage temp artifact lifecycle boundaries, and moved provider generated media defaults to `data/temp/generated_media`.
- `M7-R34-t2i-render-boundary`: added `astrbot-render` with typed T2I render requests/results, strategy selection, artifact descriptors, and an AstrBot-style user-overrides-builtin template catalog while keeping RespondStage/WebChat routes out of rendering.

Success criteria:
- New platforms/providers are registry-only additions.
- Dashboard never reaches into EventBus/Pipeline internals.

### M8 Dashboard Rewrite

Depends on: M1-M7 (frozen `astrbot-web::management` contract during the rewrite)

Status: planned

Reference:
- `E:/Playground/astrbot-rs/dashboard/` (legacy vanilla JS, deleted in this milestone)
- `E:/Playground/astrbot-rs/crates/astrbot-web/src/management/` (26 modules, contract frozen)
- `E:/Playground/astrbot-rs/crates/astrbot-runtime/src/dashboard_assets.rs` (`DashboardAssetSource`, `DASHBOARD_INDEX_ROUTES`)
- `E:/Playground/Astrbot/dashboard/` (Vue 3 reference for feature parity)
- Scratch: `.workflow/scratch/dashboard-next-design-2026-05-19/`

Decision summary:
- Stack: Solid + Vite 5 + TypeScript strict + `@kobalte/core` + CSS variables/CSS Modules.
- TS DTOs are generated from Rust via `ts-rs`; CI enforces `git diff --exit-code dashboard-next/src/api/dto/`.
- Hash routing preserved so `is_dashboard_index_route` SPA fallback keeps working unchanged.
- `DashboardAssetSource::NextDist` added; `DASHBOARD_INDEX_ROUTES` extended with `/mcp`, `/api-keys`, `/observability`, `/t2i-templates`.
- Legacy `dashboard/` directory is removed as part of planning; no dashboard-legacy keepalive.
- One-shot rewrite — no dual-track migration.

Deliverables:
- `dashboard-next/` Solid + Vite scaffold with strict TS, lint, and Playwright e2e.
- Functional parity with AstrBot Vue dashboard across 28 routes (24 legacy + 4 new).
- `ts-rs` workspace dep + DTO export trigger + CI drift gate.
- Runtime asset wiring for `NextDist` and `dashboard_next_dist_dir` path layout entry.
- Phase 9 cutover that flips `DashboardAssetSource` default to `NextDist` and trims `BundledDist`.

Backlog (mapped to `.workflow/scratch/dashboard-next-design-2026-05-19/plan.json`):
1. `M8-T1-dashboard-next-scaffold`: package, vite, tsconfig, App, NextDist asset source, 3 pilot ts-rs DTOs.
2. `M8-T2-appshell-auth-i18n-base-components`: AppShell, router, i18n, theme tokens, base components, Login.
3. `M8-T3-readonly-pages`: Overview, Trace, Console (SSE), Settings, About.
4. `M8-T4-config-providers-platforms`: schema-driven Config tree editor, Providers, Platforms with CodeMirror 6.
5. `M8-T5-chat-core`: Chat, ChatBox, Conversation, MessagePartsRenderer, Markdown + KaTeX.
6. `M8-T6-extensions`: Plugins (installed + market), Skills, Tools, SubAgent.
7. `M8-T7-knowledge-base`: KBList, KBDetail, DocumentDetail, resumable Upload.
8. `M8-T8-persona-cron-sessions-projects`: persona folder tree with drag-drop, Cron, Sessions, ChatUI Projects.
9. `M8-T9-ops-and-new-pages`: Backup, Update, MCP, ApiKeys, Observability, T2I Templates.
10. `M8-T10-cutover`: Playwright e2e, bundle analysis, code-split, flip NextDist default, verify legacy removal.

Success criteria:
- `dashboard-next/dist` served via `DashboardAssetSource::NextDist` matches AstrBot Vue dashboard feature surface.
- `git ls-files dashboard/` returns empty (legacy directory removed).
- DTO drift check green: `cargo test -p astrbot-web ts_export && git diff --exit-code dashboard-next/src/api/dto/`.
- First-screen bundle ≤ 250 KB gzipped; Playwright covers login + 5 highest-traffic pages.
- `M7-T3-dashboard-api-and-ui` deferred entry resolved by this milestone.

## Immediate Backlog

1. `M7-R35-observability-trace-boundary`: introduce trace, log buffer, status event, and management-log boundaries.
2. `M7-R36-event-bus-routing-boundary`: split event bus dispatch, scheduler routing, event outline logging, and backpressure policy before multi-config pipelines grow.
3. `M7-R37-provider-selection-boundary`: define selected/default provider state and change notifications outside dashboard/runtime internals.
4. `M7-R38-conversation-history-boundary`: introduce conversation and platform message history domain services before persistent memory parity.
5. `M7-R39-persona-skill-boundary`: define persona manager, skill catalog, and prompt composition boundaries before main-agent parity.
6. `M7-R40-cron-proactive-boundary`: isolate scheduled jobs and proactive agent wake flows from runtime handle and WebChat transport.
7. `M7-R41-subagent-handoff-boundary`: define subagent config, handoff registration, provider override, and tool bridge boundaries.
8. `M7-R42-computer-use-boundary`: define computer-use sandbox booters, capability components, tool exposure, prompt fragments, and skill-sync boundaries.
9. `M7-R43-plugin-dependency-environment-boundary`: separate plugin dependency installer, isolated import environment, conflict classification, and secret redaction.
10. `M7-R44-platform-api-client-boundary`: define platform API client, retry/rate-limit, WebSocket error, and rich-event normalization boundaries.
11. `M7-R45-quote-forward-parser-boundary`: extract quoted and forwarded message parser boundaries before quote/multimodal parity expands.
12. `M7-R46-live-agent-feedback-boundary`: separate live agent streaming, tool-status, stop-signal, and TTS feedback orchestration.
13. `M7-R47-provider-non-chat-protocol-boundary`: extract remaining embedding, STT, TTS, and rerank protocol DTO/parsers from concrete provider adapters.
14. `M7-R48-xinference-model-resolver-boundary`: deduplicate Xinference model UID cache, running lookup, optional launch, and disabled auto-launch policy.
15. `M7-R49-provider-manager-bucket-boundary`: extract reusable provider manager bucket assembly and accessor helpers.
16. `M7-R50-plugin-test-boundary`: split plugin tests by registry, filter, loader, manifest/SDK, dependency, tool, and sandbox behavior.
17. `M7-R51-runtime-policy-config-boundary`: split runtime policy config into per-policy modules before `platform_settings` parity grows.
18. `M7-R52-provider-tts-factory-boundary`: separate TTS option normalization and provider-specific factory config mapping.
19. `M7-R53-management-auth-api-key-boundary`: define dashboard JWT auth, OpenAPI API-key scopes, route middleware, and API-key storage ports.
20. `M7-R54-management-config-mutation-boundary`: separate dashboard config mutation, validation, config file handling, UMOP routing, and restart/reload policy.
21. `M7-R55-plugin-market-update-boundary`: define plugin marketplace source/cache/package/update boundaries before plugin install/update dashboard parity.
22. `M7-R56-session-concurrency-boundary`: introduce session waiter, per-session lock, and active-event interruption boundaries for multi-turn plugin/agent flows.
23. `M7-R57-realtime-chat-gateway-boundary`: separate realtime WebSocket/OpenAPI chat, audio frame, attachment, and subscription gateway concerns.
24. `M7-R58-file-token-static-boundary`: define file-token, static asset, and scoped download services for dashboard files, plugin logos, backups, and uploads.
25. `M7-R59-long-term-memory-boundary`: isolate long-term memory, active-reply policy, image-caption provider calls, and memory request decoration.
26. `M7-R60-external-agent-runner-boundary`: define Coze/Dify/DashScope/DeerFlow-style external agent connectors and stream mapping outside normal provider adapters.
27. `M7-R61-mcp-transport-bridge-boundary`: implement MCP stdio/HTTP transport runtime, tolerant JSON-RPC framing, process supervision, reconnect, and bridge registration.
28. `M7-R62-backup-job-service-boundary`: separate backup upload sessions, task progress, manifest, import, and export services from web route handlers.
29. `M7-R63-platform-outbound-routing-boundary`: define session scene/message-id/sender binding state for platform outbound and proactive-send routing.
30. `M7-R64-skill-package-boundary`: separate skill package install/delete, sandbox cache, activation, and prompt inventory rendering from persona/plugin logic.
31. `M7-R65-pipeline-preprocess-boundary`: define pre-ack reaction, media path mapping, and STT normalization before process/provider stages.
32. `M7-R66-internal-tool-provider-boundary`: separate internal tool source metadata, registration, and dashboard management from plugin/MCP tool paths.
33. `M7-R67-chatui-project-boundary`: introduce ChatUI project CRUD, session membership, and ownership policy outside conversation history routes.
34. `M7-R68-provider-tts-streaming-audio-boundary`: define streaming TTS text/audio queues and live feedback integration apart from file-based TTS.
35. `M7-R69-storage-schema-catalog-boundary`: split storage schema primitives and AstrBot main DB table-family builders before concrete SQL backends grow.
36. `M7-R70-mcp-wire-types-boundary`: split MCP errors, names, URIs, JSON values/schema, pagination, and future JSON-RPC wire primitives.
37. `M7-R71-kb-ingestion-index-boundary`: introduce KB ingestion, document repository, media store, indexing job, and vector persistence boundaries.
38. `M7-R72-agent-run-context-hook-boundary`: define agent hook dispatch, run context, response event DTOs, message wrappers, and tool-image cache ports.
39. `M7-R73-provider-response-metadata-boundary`: define provider response metadata, token usage, reasoning signatures, and tool-call payload boundaries.
40. `M7-R74-platform-identity-membership-boundary`: introduce platform identity, member profile, group metadata, and permission resolution boundaries.
41. `M7-R75-session-rule-preference-boundary`: define session rule, scoped preference, session group, and batch management boundaries.
42. `M7-R76-media-normalization-boundary`: introduce media input normalization, data URL conversion, safe download, and provider attachment resolver boundaries.
43. `M7-R77-maintenance-operation-boundary`: separate runtime maintenance, release update, migration, and package install operation boundaries.
44. `M7-R78-t2i-implementation-boundary`: separate concrete T2I network, local raster, markdown, font, and endpoint strategy boundaries.
45. `M7-R79-network-download-boundary`: define shared download, TLS, proxy, progress, and cache boundaries for provider/platform/render/update flows.
46. `M7-R80-kb-management-api-boundary`: separate KB management API, provider preflight, upload task, and progress boundaries.
47. `M7-R81-metrics-usage-boundary`: introduce metrics, token usage accounting, installation identity, and stats sink boundaries.
48. `M7-R82-tool-reference-boundary`: define tool output reference, citation extraction, and route-independent refs boundaries.
49. `M7-T2-provider-parity`: resume remaining voice/provider adapters, concrete media conversion adapters, and provider-specific runtime option schemas after the decoupling gates are green.
50. `M7-T3-dashboard-api-and-ui`: superseded by `M8 Dashboard Rewrite` (one-shot Solid + Vite + TS rewrite tracked in `.workflow/scratch/dashboard-next-design-2026-05-19/`).

## Deferred Until Pipeline Contracts Stabilize

- More WebChat upper-layer API breadth.
- Dashboard management APIs and UI — superseded by **M8 Dashboard Rewrite** (one-shot Solid + Vite + TS rewrite in `.workflow/scratch/dashboard-next-design-2026-05-19/`); legacy `dashboard/` directory removed in planning.
- Full platform adapter parity.
- Provider tool-call orchestration.
- Persistent attachment storage and WebChat auth.
- Persistent storage backend selection until storage ports are introduced.
- Dashboard management mutations/UI until provider/platform/runtime contracts are broader and stable. → Lifted: M8 Dashboard Rewrite drives the UI rewrite with the `management/*` HTTP contract frozen during the rewrite.
- Observability APIs until that boundary exists outside chat transport and pipeline response delivery.
- Provider manager bucket helpers, plugin test modules, runtime policy config modules, and TTS option normalization are now recorded as follow-up growth guards before those surfaces expand.
- Dashboard auth/API-key, config mutation/UMOP routing, plugin marketplace/update, session concurrency, realtime chat gateway, and file-token/static serving are now recorded as `TASK-058` through `TASK-063`.
- Long-term memory, external agent runner connectors, MCP transport runtime, backup job services, platform outbound routing state, and skill package lifecycle are now recorded as `TASK-064` through `TASK-069`.
- Pipeline preprocessing, internal tool providers, ChatUI project membership, and streaming TTS audio queues are now recorded as `TASK-070` through `TASK-073`.
- Storage schema catalog modules, MCP wire primitives, KB ingestion/index jobs, and agent hook/run-context side channels are now recorded as `TASK-074` through `TASK-077`.
- Provider response metadata, platform identity/membership, scoped session rules, media normalization, and maintenance operations are now recorded as `TASK-078` through `TASK-082`.
- T2I rendering is now represented by `astrbot-render`; real browser/local rasterization and remote T2I API clients remain behind that renderer trait.
- Concrete T2I implementations, shared network download/TLS/progress, KB management APIs, metrics/usage accounting, and tool reference extraction are now recorded as `TASK-083` through `TASK-087`.
