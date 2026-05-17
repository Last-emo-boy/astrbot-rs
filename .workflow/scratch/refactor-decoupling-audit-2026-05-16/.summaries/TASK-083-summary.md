# TASK-083 Summary

Completed at: 2026-05-17T15:41:21+08:00

## Scope

Split concrete T2I network, local raster planning, markdown parsing, font selection, text measurement, endpoint fallback, and artifact writing boundaries inside `astrbot-render`.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/utils/t2i/renderer.py`
- `E:/Playground/Astrbot/astrbot/core/utils/t2i/network_strategy.py`
- `E:/Playground/Astrbot/astrbot/core/utils/t2i/local_strategy.py`
- `E:/Playground/Astrbot/astrbot/core/utils/t2i/template_manager.py`

## Changes

- Added `network.rs` with normalized T2I endpoints, official endpoint descriptors, fallback catalog, client trait, and network renderer.
- Added `local.rs` with local template renderer compatibility, local markdown renderer, raster plans, and temp artifact writer using `TempArtifactRoot`.
- Added `markdown.rs` with route-independent markdown block and inline span modeling for headings, quotes, lists, code blocks, images, and inline styles.
- Added `font.rs` with font catalog, style requests, fallback selection, and text wrapping/measurement boundaries.
- Slimmed `t2i.rs` back to facade DTOs, trait, and template helper functions.
- Re-exported the new render boundaries from `astrbot-render`.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-render`
- `cargo clippy -p astrbot-render -- -D warnings`
- `cargo test --workspace`

## Next

Next pending task is `TASK-084`.
