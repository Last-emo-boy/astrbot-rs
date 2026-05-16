# M7-T2o Provider Parity: Xinference Speech-To-Text Provider

## Reference

- AstrBot registers `xinference_stt` with `ProviderType.SPEECH_TO_TEXT`.
- AstrBot `ProviderXinferenceSTT` lists running Xinference models, resolves the UID by `model_name`, optionally launches the model with `model_type="audio"`, then calls the OpenAI-compatible `/v1/audio/transcriptions` endpoint with `model=<model_uid>` and `file=audio.wav`.
- AstrBot downloads HTTP audio itself, detects Tencent/SILK/AMR audio, converts it through Tencent helper utilities, and returns the transcription text.

## Rust Decision

- Add `XinferenceSpeechToTextProvider` behind the existing `SpeechToTextProvider` trait.
- Register AstrBot-compatible provider type `xinference_stt` through `ProviderRegistry::with_builtin_providers`.
- Reuse the Xinference Rerank pattern: lazy model UID resolution, optional model launch, compatible parsing for UID-map and OpenAI-like `data` model list responses.
- Keep HTTP/HTTPS audio downloads auth-isolated from the provider client so Xinference API keys are not sent to third-party audio URLs.
- Use the shared `AudioInputLoader`/`AudioMediaConverter` boundary for local/HTTP audio loading and SILK/AMR/Tencent detection. The default converter remains unsupported until a concrete media conversion adapter is designed.
- Keep runtime, pipeline voice flow, and dashboard wiring deferred until non-chat provider runtime config is designed.

## Verification

- PASS: `cargo test -p astrbot-provider`
- PASS: `cargo fmt --all --check`
- PASS: `cargo test --workspace`
- PASS: `cargo clippy --workspace -- -D warnings`
- PASS: `cargo run -p astrbot-cli`
- PASS: `.workflow` JSON parse check
