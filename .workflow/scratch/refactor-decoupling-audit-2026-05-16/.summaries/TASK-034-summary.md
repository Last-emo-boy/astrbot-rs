# TASK-034 Summary

`TASK-034` is completed. Provider voice/media tests now share temporary media fixtures and generated-audio cleanup helpers instead of repeating timestamp path creation and manual cleanup in each integration test.

Implemented boundary:

- `tests/support/media_fixture.rs`: `TempAudioFile`, `TempOutputDir`, and `GeneratedAudioFile` RAII helpers.
- `tests/support/mod.rs`: exports the media fixture support module.

Updated tests:

- `openai_tts.rs`, `gemini_tts.rs`, `volcengine_tts.rs`, `minimax_tts.rs`, and `gsvi_tts.rs` now use `TempOutputDir`.
- `openai_stt.rs`, `xinference_stt.rs`, `audio_media.rs`, and `provider_registry/speech.rs` now use `TempAudioFile`.
- `provider_registry/tts.rs` now uses `GeneratedAudioFile` to centralize generated audio cleanup.

AstrBot comparison:

- AstrBot TTS providers write generated audio through `get_astrbot_temp_path()` with provider-specific filenames. Rust provider tests now mirror that idea at test-support level: generated media paths are a fixture concern, not behavior assertions scattered through provider tests.
- Provider request/response assertions were kept unchanged; the refactor only moves temp media setup and cleanup out of behavior tests.

Verification passed:

- `cargo fmt --all --check`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

Next concrete task: `M7-R30-provider-media-artifact-boundary` / `TASK-035`.
