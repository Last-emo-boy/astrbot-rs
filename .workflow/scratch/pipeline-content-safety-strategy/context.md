# Pipeline Content Safety Strategy

## AstrBot Reference

AstrBot `ContentSafetyCheckStage`:

- reads `content_safety` config in `initialize`;
- delegates checks to `StrategySelector`;
- currently checks text only;
- stops unsafe events;
- if the event was wake/at command, sets a user-facing blocked-content result before stopping.

`StrategySelector` enables internal keyword and Baidu AIP strategies. The internal keyword strategy iterates configured keywords and fails on the first match.

## Rust Boundary

M2-T3 should add the strategy boundary without external services:

- `ContentSafetyStrategy` trait lives in `astrbot-pipeline`.
- `KeywordContentSafetyStrategy` is the first concrete strategy.
- `ContentSafetyCheckStage` runs after `rate_limit` and before plugin/provider/respond.
- Unsafe wake/at messages produce a blocked-content result for `RespondStage`; unsafe non-wake messages stop silently.

## Deferred

- Baidu AIP or other external review providers.
- Response-side safety checks from AstrBot `ResultDecorateStage`.
- Regex-compatible keyword semantics if needed by config migration.

## Result

- Added `ContentSafetyStrategy`, `ContentSafetyVerdict`, `ContentSafetyConfig`, and `KeywordContentSafetyStrategy`.
- Added `ContentSafetyCheckStage` after `rate_limit` and before plugin/provider/respond.
- Updated `PluginStage` to skip when earlier stages already set a result, preserving safety rejection messages.
- Runtime config now wires internal keyword safety strategy into `PipelineContext`.
- M2 policy stages are complete; current Maestro state moves to `M3-T1-unified-process-stage`.
