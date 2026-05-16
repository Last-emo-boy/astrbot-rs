# TASK-037 Summary

Completed: 2026-05-16T21:01:47+08:00

Scope:
- Introduce a dedicated multimodal request preparation boundary in `astrbot-agent`.
- Keep image captioning, quoted image fallback, and provider modality filtering out of `ProcessStage` and concrete provider adapters.
- Preserve provider protocol modules as serialization/parsing boundaries only.

Changes:
- Added `crates/astrbot-agent/src/multimodal/{mod.rs,image_caption.rs,quoted_image.rs,capability_filter.rs}`.
- Added `ImageCaptioner`, `ImageCaptionRequestDecorator`, `ImageCaptionConfig`, and `ChatProviderImageCaptioner`.
- Added `QuotedImageAttachmentPolicy` for quoted fallback image refs and attachment text.
- Added `ProviderModalitySupport`, `ModalityFallbackPolicy`, and `ModalityFilterRequestDecorator` for unsupported image/tool-use handling.
- Re-exported the multimodal boundary from `crates/astrbot-agent/src/lib.rs`.

AstrBot reference:
- `_request_img_caption` and `_ensure_img_caption` in `astrbot/core/astr_main_agent.py`.
- `_append_quoted_image_attachment` and quoted image fallback handling in `astrbot/core/astr_main_agent.py`.
- `_modalities_fix` and `_sanitize_context_by_modalities` in `astrbot/core/astr_main_agent.py`.
- `image_refs.py` and `provider/entities.py` for quoted image refs and request part shape.

Verification:
- `cargo fmt --all --check`
- `cargo test -p astrbot-agent`
- `cargo clippy -p astrbot-agent -- -D warnings`
- `cargo test -p astrbot-pipeline`
- `cargo test -p astrbot-provider`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next:
- `M7-R33-path-temp-artifact-boundary` / `TASK-038`.
