# Pipeline Stage Registry And Order Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/pipeline/stage.py` registers stage classes.
- `E:/Playground/Astrbot/astrbot/core/pipeline/stage_order.py` defines deterministic stage ordering.
- `E:/Playground/Astrbot/astrbot/core/pipeline/scheduler.py` sorts registered stages and initializes instances.

## Current Rust State

- Runtime manually builds `PipelineScheduler::new(context).with_stage(PluginStage).with_stage(ProviderStage).with_stage(RespondStage)`.
- `PipelineScheduler` stores ordered stage objects, but there is no registry/factory boundary yet.

## Decision

Add `PipelineStageRegistry` with explicit order values and built-in plugin/provider/respond stage factories. Runtime should construct the default scheduler through this registry while existing direct `with_stage` tests remain valid.

## Completed

- Added `PipelineStageRegistry` with duplicate rejection, deterministic order, and stage factories.
- Added built-in stage registration for `plugin`, `provider`, and `respond`.
- Added `PipelineScheduler::from_registry()` and `stage_names()`.
- Runtime now builds its default scheduler from `PipelineStageRegistry::with_builtin_stages()`.
- Existing direct `with_stage()` construction remains available for tests/custom pipelines.

## Verification Results

- `cargo test -p astrbot-pipeline`
- `cargo test -p astrbot-runtime`
