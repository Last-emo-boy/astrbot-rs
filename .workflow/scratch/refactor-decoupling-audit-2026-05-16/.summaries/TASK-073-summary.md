# TASK-073 Summary

Completed at: 2026-05-17T13:18:31+08:00

## Scope

Defined provider-neutral streaming TTS audio queue and live feedback boundaries, following AstrBot's separation between file-based `get_audio` and stream-oriented `get_audio_stream`.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/provider/provider.py`
- `E:/Playground/Astrbot/astrbot/core/provider/sources/genie_tts.py`
- `E:/Playground/Astrbot/astrbot/core/provider/sources/minimax_tts_api_source.py`

## Changes

- Added `TextToSpeechAudioChunk` and `TextToSpeechAudioQueueItem` under `astrbot-provider/src/tts/audio_queue.rs` to model text-correlated audio chunks, terminal markers, and stream errors without provider protocol coupling.
- Added `TextToSpeechStreamRequest`, `TextToSpeechAudioStream`, and `TextToSpeechStreamProvider` under `astrbot-provider/src/tts/stream.rs`.
- Added `QueuedTextToSpeechAudioStream` for deterministic stream tests and `FileSynthesisTextToSpeechStreamProvider` to adapt existing file-based TTS providers without merging the traits.
- Re-exported the new TTS stream contracts from `astrbot-provider` while preserving existing `TextToSpeechProvider` API.
- Added `LiveTtsStreamFeedbackBridge` in `astrbot-agent/src/feedback/tts_stream.rs` so live feedback can consume provider-neutral audio streams instead of concrete adapters.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-provider`
- `cargo test -p astrbot-agent`
- `cargo clippy --workspace -- -D warnings`

## Next

Next pending task is `TASK-074`.
