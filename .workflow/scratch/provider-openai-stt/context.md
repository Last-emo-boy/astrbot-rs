# M7-T2n Provider Parity: OpenAI Whisper Speech-To-Text Provider

## Reference

- AstrBot registers `openai_whisper_api` with `ProviderType.SPEECH_TO_TEXT`.
- AstrBot `ProviderOpenAIWhisperAPI` downloads HTTP audio to a temporary file, optionally converts Tencent/SILK/AMR audio to WAV, then calls OpenAI-compatible audio transcription with `model` and `file=("audio.wav", ...)`.
- AstrBot returns `result.text` through the STT provider boundary.

## Rust Decision

- Add `OpenAiSpeechToTextProvider` behind the existing `SpeechToTextProvider` trait.
- Register AstrBot-compatible provider type `openai_whisper_api` through `ProviderRegistry::with_builtin_providers`.
- Keep STT as a provider-crate-only capability for now; runtime, pipeline voice flow, and dashboard wiring remain deferred until non-chat provider config is designed.
- Normalize registry construction to an OpenAI `/v1` base URL while keeping direct provider config explicit.
- Download HTTP/HTTPS audio with a separate unauthenticated client so provider API keys are not leaked to third-party audio URLs.
- Use the shared `AudioInputLoader`/`AudioMediaConverter` boundary for local/HTTP audio loading and SILK/AMR/Tencent detection. The default converter remains unsupported until a concrete media conversion adapter is designed.

## Verification

- PASS: `cargo fmt --all --check`
- PASS: `cargo test -p astrbot-provider`
- PASS: `cargo test --workspace`
- PASS: `cargo clippy --workspace -- -D warnings`
- PASS: `cargo run -p astrbot-cli`
- PASS: `.workflow` JSON parse check
