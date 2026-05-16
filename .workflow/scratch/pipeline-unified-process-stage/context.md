# Pipeline Unified Process Stage

## AstrBot Reference

AstrBot `ProcessStage` is the unified processing facade after policy checks:

- `ProcessStage.initialize` builds `StarRequestSubStage` and `AgentRequestSubStage`.
- `ProcessStage.process` first runs activated Star/plugin handlers from event extras.
- If a plugin yields `ProviderRequest`, it stores it on the event and delegates to the agent sub-stage.
- If no plugin send/result has happened and the event is an at/wake command, it falls back to the agent/LLM sub-stage.
- Provider processing is skipped when `provider_settings.enable` is false or session-level LLM processing is disabled.

## Rust Boundary

M3-T1 keeps the Rust version deliberately smaller:

- `ProcessStage` lives in `astrbot-pipeline` and coordinates the existing `PluginRegistry` and `ChatProvider`.
- The default built-in order becomes `wake -> whitelist -> session_status -> rate_limit -> content_safety -> process -> respond`.
- `PluginStage` and `ProviderStage` remain available for focused tests and custom schedulers.
- Full `ProviderRequest`, activated-handler extras, and richer fallback policy remain deferred to M3-T2/M3-T3.

## Verification Targets

- Plugin command results suppress provider fallback.
- Provider fallback runs when no plugin result/stop exists and text/image content is present.
- Missing provider skips LLM calls without breaking plugin replies.
- Default builder and runtime report `process` instead of separate `plugin`/`provider`.

## Result

- Added `ProcessStage` as the default M3 processing facade.
- Shared the existing plugin and provider logic through stage-internal helpers to avoid behavior drift.
- Updated built-in order to `wake -> whitelist -> session_status -> rate_limit -> content_safety -> process -> respond`.
- Kept `PluginStage` and `ProviderStage` exported for custom schedulers and focused compatibility tests.
- Added process-stage tests for plugin suppression, provider fallback, image-only fallback, and missing-provider skip behavior.

## Verification

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
- `.workflow/**/*.json` parsed with `ConvertFrom-Json`
