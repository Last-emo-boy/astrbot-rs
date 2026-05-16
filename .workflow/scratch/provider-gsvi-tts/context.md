# M7-T2u GSVI TTS Provider

Reference: `E:/Playground/Astrbot/astrbot/core/provider/sources/gsvi_tts_source.py`

AstrBot registers `gsvi_tts_api` as `ProviderType.TEXT_TO_SPEECH`. The provider calls `{api_base}/tts` with query parameters `text`, optional `character`, and optional `emotion`; a successful response body is written to a temporary `.wav` file.

Rust mapping:

- `GsviTextToSpeechProvider` implements `TextToSpeechProvider`, not `ChatProvider`.
- `GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE` remains `gsvi_tts_api` for AstrBot-compatible registry lookup.
- `character` maps from provider option `character`, with `TextToSpeechProviderConfig.voice` as a convenient typed alias; `emotion` remains adapter-specific in `provider_options`.
- Runtime maps `RuntimeTextToSpeechProviderConfig::gsvi` into the provider manager TTS bucket. `PipelineContext` remains chat-only until a future voice flow explicitly consumes `TextToSpeechProvider`.

Coverage:

- `crates/astrbot-provider/tests/gsvi_tts.rs` covers query encoding, WAV output, error response mapping, empty audio, empty input, and provider type.
- `crates/astrbot-provider/tests/provider_registry.rs` covers builtin registration and manager construction from registry config.
- `crates/astrbot-runtime/src/lib.rs` tests provider-specific runtime mapping.
