# Pipeline Default Builder Context

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/pipeline/bootstrap.py`
- `E:/Playground/Astrbot/astrbot/core/pipeline/scheduler.py`
- `E:/Playground/Astrbot/astrbot/core/pipeline/stage_order.py`

## Current Rust State

- `PipelineStageRegistry` owns stage factories and deterministic order.
- `PipelineScheduler::initialize()` calls stage initialize hooks.
- Runtime still wires registry + scheduler + initialize directly.

## Decision

Introduce a named `DefaultPipelineBuilder` that owns the default registry, builds the scheduler, and initializes it. Runtime should delegate default pipeline construction to this builder.

## Completed

- Added `DefaultPipelineBuilder` to own built-in registry construction, scheduler creation, and initialization.
- Runtime now delegates default pipeline construction to `DefaultPipelineBuilder`.
- Added tests for default builder stage order and initialization-before-return behavior.
- Added scheduler stop-control coverage before closing M1.

## Verification Results

- `cargo test -p astrbot-pipeline`
- `cargo test -p astrbot-runtime`
