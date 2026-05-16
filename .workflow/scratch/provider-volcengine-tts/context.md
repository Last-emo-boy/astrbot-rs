# M7-T2s Volcengine TTS Provider

Reference: `E:/Playground/Astrbot/astrbot/core/provider/sources/volcengine_tts.py`

AstrBot registers `volcengine_tts` as `ProviderType.TEXT_TO_SPEECH`. The provider reads `api_key`, `appid`, `volcengine_cluster`, `volcengine_voice_type`, `volcengine_speed_ratio`, `api_base`, and `timeout`; sends `Authorization: Bearer; {api_key}`; serializes the app/user/audio/request payload with `encoding = "mp3"`; decodes response field `data` as base64; and writes a temporary `.mp3` file.

Rust mapping:

- `VolcengineTextToSpeechProvider` implements `TextToSpeechProvider`, not `ChatProvider`.
- `VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE` remains `volcengine_tts` for AstrBot-compatible registry lookup.
- Shared TTS fields stay in `TextToSpeechProviderConfig`; Volcengine-specific fields use `provider_options` so the common TTS schema does not absorb adapter-only keys.
- Runtime maps `RuntimeTextToSpeechProviderConfig::volcengine` into the provider manager TTS bucket. `PipelineContext` remains chat-only until a future voice/RAG flow explicitly consumes non-chat provider traits.

Coverage:

- `crates/astrbot-provider/tests/volcengine_tts.rs` covers request payload, auth header, MP3 output, error response mapping, missing `data`, empty input, and provider type.
- `crates/astrbot-provider/tests/provider_registry.rs` covers builtin registration and manager construction from registry config.
- `crates/astrbot-runtime/src/lib.rs` tests provider-specific runtime mapping through `provider_options`.
