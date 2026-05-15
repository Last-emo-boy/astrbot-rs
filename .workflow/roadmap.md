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

Success criteria:
- New platforms/providers are registry-only additions.
- Dashboard never reaches into EventBus/Pipeline internals.

## Immediate Backlog

1. `M7-T2-provider-parity`: continue with remaining voice/provider adapters, concrete media conversion adapters, and provider-specific runtime option schemas before wiring Dashboard.
2. `M7-T3-dashboard-api-and-ui`: defer until provider/platform runtime contracts are broader and stable.

## Deferred Until Pipeline Contracts Stabilize

- More WebChat upper-layer API breadth.
- Dashboard management APIs and UI.
- Full platform adapter parity.
- Provider tool-call orchestration.
- Persistent attachment storage and WebChat auth.
