# TASK-070 Summary

Completed at: 2026-05-17T12:16:09+08:00

## Scope

Added an AstrBot-inspired pipeline preprocess boundary before policy/process handling.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/pipeline/preprocess_stage/stage.py`

## Changes

- Added `PreprocessStage` with optional pre-ack reaction, media path mapping, and Record-to-Plain STT normalization.
- Added `PreprocessConfig`, `PreAckReactionSink`, `PreprocessPathMapper`, `PrefixPathMapper`, and `SpeechToTextPreprocessConfig` under pipeline context.
- Inserted builtin preprocess stage after wake and before whitelist/session/rate/content/process stages so it can consume wake-command metadata while still running before process/provider handling.
- Added `MessageChain::components_mut()` as the narrow mutable message-chain surface required by preprocessing.
- Added platform adapter common `PlatformPathMapping` and `PlatformPathMappingRules` so path mapping policy stays outside concrete adapters and core message types.

## Verification

- `cargo test -p astrbot-pipeline`
- `cargo test -p astrbot-platform`
- `cargo test -p astrbot-core`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all --check`

## Next

Next pending task is `TASK-071`.
