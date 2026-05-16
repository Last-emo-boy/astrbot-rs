# Pipeline Provider Fallback Policy

## AstrBot Reference

AstrBot gates LLM processing in multiple places:

- `ProcessStage.process` skips provider work when `provider_settings.enable` is false.
- Default fallback only runs when no plugin send/result happened and the event is at/wake.
- `AgentRequestSubStage.process` checks session-level LLM enablement before invoking the agent.
- `InternalAgentSubStage.process` catches provider/agent errors and sends a generic or persona-specific error message.

## Rust Boundary

M3-T3 should make fallback behavior explicit without implementing session memory:

- `ProviderFallbackConfig` lives in `astrbot-pipeline::PipelineContext`.
- Runtime config wires provider fallback enabled/require-wake/error-message values into the pipeline.
- Explicit plugin-generated `ProviderRequest` bypasses fallback wake gating because it is already an intentional plugin action.
- Provider errors can be converted into a configured generic response before `RespondStage`.

## Verification Targets

- Disabled fallback skips provider calls even when a provider exists.
- `require_wake` blocks implicit fallback for non-wake events.
- Plugin-generated provider requests still run when `require_wake` is enabled.
- Provider errors produce configured generic replies without exposing provider internals.

## Result

- Added `ProviderFallbackConfig` to `PipelineContext`.
- Provider fallback can now be disabled without removing configured providers.
- Implicit fallback can require wake/at metadata while explicit plugin-generated `ProviderRequest` still runs.
- Provider errors can map to a configured generic `MessageEventResult` for `RespondStage`.
- Runtime config now wires `provider_fallback` into the default pipeline.
- M3 Process Stage success criteria are complete.

## Verification

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
- `.workflow/**/*.json` parsed with `ConvertFrom-Json`
