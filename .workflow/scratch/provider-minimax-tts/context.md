# M7-T2t MiniMax TTS Provider

Reference: `E:/Playground/Astrbot/astrbot/core/provider/sources/minimax_tts_api_source.py`

AstrBot registers `minimax_tts_api` as `ProviderType.TEXT_TO_SPEECH`. The provider sends a streaming JSON request to `api_base?GroupId={group_id}` with bearer auth, `stream = true`, `language_boost`, `voice_setting`, `audio_setting`, and optional `timber_weights`. SSE messages provide hex-encoded MP3 chunks at `data.audio`; AstrBot concatenates decoded chunks and writes a temporary `.mp3`.

Rust mapping:

- `MiniMaxTextToSpeechProvider` implements `TextToSpeechProvider`, not `ChatProvider`.
- `MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE` remains `minimax_tts_api` for AstrBot-compatible registry lookup.
- Shared TTS fields stay in `TextToSpeechProviderConfig`; MiniMax-only keys use `provider_options` so the common TTS schema does not absorb adapter-specific fields.
- Runtime maps `RuntimeTextToSpeechProviderConfig::minimax` into the provider manager TTS bucket. `PipelineContext` remains chat-only until a future voice flow explicitly consumes `TextToSpeechProvider`.

Coverage:

- `crates/astrbot-provider/tests/minimax_tts.rs` covers request payload, auth header, SSE hex audio output, timber_weights mode, error response mapping, missing audio data, empty input, and provider type.
- `crates/astrbot-provider/tests/provider_registry.rs` covers builtin registration and manager construction from registry config.
- `crates/astrbot-runtime/src/lib.rs` tests provider-specific runtime mapping through `provider_options`.
