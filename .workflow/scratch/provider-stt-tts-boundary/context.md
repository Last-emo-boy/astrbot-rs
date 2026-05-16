# M7-T2k Provider Parity: STT/TTS Provider Boundary

## Reference

- AstrBot defines `STTProvider.get_text(audio_url)` separately from chat providers.
- AstrBot defines `TTSProvider.get_audio(text)` and `support_stream()` separately from chat providers.
- AstrBot `ProviderManager` keeps `stt_provider_insts` and `tts_provider_insts` in their own capability buckets, with current/default selection independent from chat providers.
- AstrBot provider types are `speech_to_text` and `text_to_speech` in `ProviderType`.

## Rust Decision

- Add provider-crate-only `SpeechToTextRequest`, `SpeechToTextResponse`, `SpeechToTextProvider`, and mock STT provider.
- Add provider-crate-only `TextToSpeechRequest`, `TextToSpeechResponse`, `TextToSpeechProvider`, and mock TTS provider.
- Add `SpeechToTextProviderConfig` and `TextToSpeechProviderConfig` plus registry factory maps.
- Extend `ProviderManager` with STT/TTS buckets, default routing, provider-id routing, streaming capability query, and terminate hooks.
- Keep concrete STT/TTS HTTP providers, runtime config, pipeline voice flow, and dashboard wiring deferred.

## Verification

- PASS: `cargo test -p astrbot-provider`
- PASS: `cargo fmt --all --check`
- PASS: `cargo test --workspace`
- PASS: `cargo clippy --workspace -- -D warnings`
- PASS: `cargo run -p astrbot-cli`
- PASS: `.workflow` JSON parse check
