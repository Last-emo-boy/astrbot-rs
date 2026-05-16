# Pipeline Result Decorate Stage

## AstrBot Reference

AstrBot `ResultDecorateStage` runs after `ProcessStage` and before `RespondStage`.
It can:

- skip empty and streaming results;
- run response-side content safety;
- call decorating hooks;
- add reply prefix;
- split long text replies;
- convert text to voice or image;
- add mention/quote decorations for supported message chains.

## Rust Boundary

M4-T1 starts with the smallest stable transform:

- `ResultDecorateStage` lives in `astrbot-pipeline`.
- `ResultDecorateConfig` is injected through `PipelineContext`.
- Default stage order becomes `wake -> whitelist -> session_status -> rate_limit -> content_safety -> process -> result_decorate -> respond`.
- First transform only prefixes the first plain component when configured.
- TTS, T2I, segmented reply, hooks, mention/quote, and response-side safety are deferred.

## Verification Targets

- Configured prefix decorates provider LLM replies before `RespondStage`.
- Empty/no-result events are unchanged.
- Default builder/runtime stage order includes `result_decorate`.
- Existing process/respond behavior remains green.

## Result

- Added `ResultDecorateConfig` to `PipelineContext`.
- Added `ResultDecorateStage` after `process` and before `respond`.
- Implemented reply-prefix decoration for the first plain result component.
- Added runtime `result_decorate` config wiring.
- Added pipeline and runtime tests for prefix decoration and empty-result behavior.

## Verification

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
- `.workflow/**/*.json` parsed with `ConvertFrom-Json`
