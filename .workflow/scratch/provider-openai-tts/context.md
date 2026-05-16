# M7-T2m Provider Parity: OpenAI Text-To-Speech Provider

## Reference

- AstrBot registers `openai_tts_api` with `ProviderType.TEXT_TO_SPEECH`.
- AstrBot `ProviderOpenAITTSAPI` uses OpenAI-compatible audio speech generation with `model`, `openai-tts-voice`, `response_format="wav"`, and writes streamed bytes into a temporary `.wav` file.
- The provider returns the generated audio file path through the TTS provider boundary.

## Rust Decision

- Add `OpenAiTextToSpeechProvider` behind the existing `TextToSpeechProvider` trait.
- Register AstrBot-compatible provider type `openai_tts_api` through `ProviderRegistry::with_builtin_providers`.
- Keep generated audio as a file path in `TextToSpeechResponse`, matching the existing Rust boundary and AstrBot's temporary-file behavior.
- Normalize registry construction to an OpenAI `/v1` base URL while keeping direct provider config explicit.
- Keep runtime/pipeline/dashboard wiring deferred until non-chat provider runtime config is designed.

## Verification

- PASS: `cargo test -p astrbot-provider`
- PASS: `cargo fmt --all --check`
- PASS: `cargo test --workspace`
- PASS: `cargo clippy --workspace -- -D warnings`
- PASS: `cargo run -p astrbot-cli`
- PASS: `.workflow` JSON parse check
