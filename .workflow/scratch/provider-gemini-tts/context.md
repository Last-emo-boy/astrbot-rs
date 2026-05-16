# M7-T2q Provider Parity: Gemini Text-To-Speech Provider

## Reference

- AstrBot registers `gemini_tts` with `ProviderType.TEXT_TO_SPEECH`.
- AstrBot `ProviderGeminiTTS` calls Gemini `generateContent` with `response_modalities=["AUDIO"]` and `speech_config.voice_config.prebuilt_voice_config.voice_name`.
- AstrBot decodes inline audio data as PCM and writes a 24 kHz mono 16-bit WAV file before returning the generated path.

## Rust Decision

- Add `GeminiTextToSpeechProvider` behind the existing `TextToSpeechProvider` trait.
- Register AstrBot-compatible provider type `gemini_tts` through `ProviderRegistry::with_builtin_providers`.
- Use Gemini REST `generateContent` so the provider stays testable with local HTTP mocks and does not introduce a Google SDK boundary.
- Decode `inlineData.data` with the workspace `base64` dependency and write a WAV header locally, matching AstrBot's PCM-to-WAV behavior.
- Keep runtime, pipeline, and dashboard wiring deferred until non-chat provider runtime config is designed.

## Verification

- PASS: `cargo test -p astrbot-provider`
- PASS: `cargo fmt --all --check`
- PASS: `.workflow` JSON parse check
- PASS: `cargo test --workspace`
- PASS: `cargo clippy --workspace -- -D warnings`
- PASS: `cargo run -p astrbot-cli`
