# Pipeline-First Roadmap Context

## User Direction

The migration should proceed from the lower-level pipeline/runtime foundations upward. Maestro should maintain current progress and future direction.

## AstrBot References

- `E:/Playground/Astrbot/astrbot/core/pipeline/scheduler.py`
- `E:/Playground/Astrbot/astrbot/core/pipeline/stage.py`
- `E:/Playground/Astrbot/astrbot/core/pipeline/stage_order.py`
- `E:/Playground/Astrbot/astrbot/core/pipeline/process_stage/stage.py`
- `E:/Playground/Astrbot/astrbot/core/pipeline/respond/stage.py`

## Current Rust Evidence

- `crates/astrbot-core/src/event.rs` keeps EventBus thin.
- `crates/astrbot-pipeline/src/scheduler.rs` executes ordered stages, but stage registration/order/lifecycle is still minimal.
- `crates/astrbot-pipeline/src/stages/plugin.rs`, `provider.rs`, and `respond.rs` are early Process/Provider/Respond equivalents.
- WebChat typed message parts are useful, but future work should be gated by pipeline-first milestones.

## Decision

Set `.workflow/roadmap.md` and `state.json.current_milestone` to a bottom-up pipeline-first migration plan. The next executable task should be `M1-T1`: stage registry/order/lifecycle parity before adding more upper-layer platform/dashboard features.
