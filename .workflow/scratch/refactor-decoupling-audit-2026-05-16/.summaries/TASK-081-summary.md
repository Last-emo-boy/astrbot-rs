# TASK-081 Summary

Completed at: 2026-05-17T15:07:50+08:00

## Scope

Introduced a provider-neutral media boundary for media input normalization, data URL parsing, MIME detection, and auth-isolated media downloads.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/provider/sources/gemini_source.py`
- `E:/Playground/Astrbot/astrbot/core/provider/sources/anthropic_source.py`
- `E:/Playground/Astrbot/astrbot/core/utils/io.py`
- `E:/Playground/Astrbot/astrbot/dashboard/routes/chat.py`

## Changes

- Added `astrbot-media` with `MediaInput`, `ResolvedMedia`, `DataUrl`, image MIME detection, and `ReqwestMediaDownloadService`.
- Centralized image data URL validation and base64 extraction outside Gemini and Anthropic protocol serializers.
- Kept provider protocol modules consuming already-normalized data URLs instead of owning remote URL, base64, and MIME parsing rules.
- Moved provider audio HTTP loading onto the shared media download service so image/audio paths share the same authorization isolation boundary.
- Added WebChat `AttachmentDescriptor::to_media_input` and platform `PlatformMediaUpload::to_media_input` so attachment/upload paths can feed the same resolver.
- Added tests for data URL parsing, `base64://` normalization, remote media download resolution, provider auth header isolation, WebChat attachment media inputs, and provider serializers.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-media`
- `cargo test -p astrbot-provider`
- `cargo test -p astrbot-platform`
- `cargo test -p astrbot-web`
- `cargo clippy --workspace -- -D warnings`

## Next

Next pending task is `TASK-082`.
