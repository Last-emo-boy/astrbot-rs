# Pipeline Provider Request Model

## AstrBot Reference

AstrBot `ProviderRequest` carries more than a prompt:

- `prompt`, `session_id`, and `image_urls` for the user request.
- `extra_user_content_parts`, `contexts`, and `system_prompt` for agent/context assembly.
- `func_tool`, `tool_calls_result`, and `model` for tool and model orchestration.
- `ProcessStage` stores plugin-yielded `ProviderRequest` on the event before delegating to the agent sub-stage.

## Rust Boundary

M3-T2 should introduce the typed request boundary without implementing the full AstrBot agent runner:

- `astrbot-core` owns an event-level `ProviderRequest` because plugins receive `MessageEvent` and must not depend on `astrbot-pipeline`.
- `astrbot-provider::ChatRequest` remains the provider trait input and can be built from `ProviderRequest`.
- `ProcessStage` and `ProviderStage` use the same conversion path for explicit plugin requests and fallback requests.
- Provider selection, context persistence, and tool execution remain placeholders for later tasks.

## Verification Targets

- Plugins can set a provider request on `MessageEvent` and `ProcessStage` sends it to the provider.
- Explicit provider request fields override fallback prompt/session/images.
- Fallback requests still preserve text and image URLs.
- Existing provider and pipeline tests remain green.

## Result

- Added `ProviderRequest` and typed support fields to `astrbot-core`.
- Added `MessageEvent::set_provider_request`, `provider_request`, `take_provider_request`, and `clear_provider_request`.
- Extended `astrbot-provider::ChatRequest` and added conversion from event-level `ProviderRequest`.
- Updated provider fallback so explicit plugin-generated requests are used before message fallback.
- OpenAI-compatible provider now consumes request-level model override, system prompt, contexts, and extra user content parts.
- Added tests for core request construction, plugin-generated request flow, and OpenAI request serialization.

## Verification

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
- `.workflow/**/*.json` parsed with `ConvertFrom-Json`
