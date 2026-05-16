# Pipeline Stage Initialize Hook Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/pipeline/stage.py`
  - `Stage.initialize(ctx)` is part of every stage contract.
- `E:/Playground/Astrbot/astrbot/core/pipeline/scheduler.py`
  - Scheduler creates ordered stage instances and initializes each stage with pipeline context.

## Current Rust State

- `PipelineStage` has `name()` and `handle()`.
- `PipelineStageRegistry` can build ordered plugin/provider/respond stages.
- `AstrbotRuntime::initialize()` is synchronous, so the first hook should be synchronous and no-op by default.

## Decision

Add `PipelineStage::initialize(&PipelineContext) -> Result<()>` as a default no-op hook. `PipelineScheduler::initialize()` should call it once for all stages, and runtime default pipeline construction should initialize the scheduler before putting it behind `Arc`.

## Completed

- `PipelineStage` now has a default no-op `initialize(&PipelineContext) -> Result<()>`.
- `PipelineScheduler::initialize()` calls stage initialization in scheduler order.
- Runtime default pipeline construction initializes the scheduler before wrapping it in `Arc`.
- Tests prove initialization order and error propagation.

## Verification Results

- `cargo test -p astrbot-pipeline`
- `cargo test -p astrbot-runtime`
