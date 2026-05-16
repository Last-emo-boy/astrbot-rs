# TASK-035 Summary - Provider Media Artifact Boundary

`TASK-035` is completed.

## Scope

Extracted generated TTS artifact path and write policy from concrete TTS adapters into `crates/astrbot-provider/src/media`.

## Result

- Added `GeneratedMediaArtifactWriter` for output directory creation, unique file naming, safe media extensions, empty-audio validation, file writes, and display-path conversion.
- Added `default_tts_output_dir()` so OpenAI, Gemini, MiniMax, Volcengine, and GSVI TTS providers share the same default temp output root.
- Updated OpenAI TTS, Gemini TTS, MiniMax TTS, Volcengine TTS, and GSVI TTS to delegate artifact writing to the shared media boundary.
- Kept provider-specific request payloads, protocol parsing, audio decoding, and Volcengine request IDs inside their adapters.

## AstrBot Reference

AstrBot TTS sources write generated audio under the shared temp path from `core/utils/astrbot_path.py` while keeping provider-specific response parsing inside each source:

- `provider/sources/openai_tts_api_source.py`
- `provider/sources/gemini_tts_source.py`
- `provider/sources/minimax_tts_api_source.py`
- `provider/sources/volcengine_tts.py`
- `provider/sources/gsvi_tts_source.py`

Rust now follows that split without introducing attachment storage or playback policy in the provider adapters.

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

## Follow-up

The next concrete boundary remains `M7-R31-streaming-strategy-boundary` / `TASK-036`. The post-task scan found no new independent task IDs beyond the current `TASK-036` through `TASK-077` backlog.
