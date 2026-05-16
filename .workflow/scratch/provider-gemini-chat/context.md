# M7-T2c Provider Parity: Gemini Chat

## Reference

- AstrBot registers `googlegenai_chat_completion` through the provider registry.
- AstrBot's Gemini provider builds Google GenAI `generate_content` calls from system instruction, user/model conversation contents, text parts, and base64 image parts.
- AstrBot treats safety and prohibited finish reasons as provider errors instead of returning empty assistant content.

## Rust Decision

- Add `GeminiProvider` as a native non-OpenAI chat provider, separate from OpenAI-compatible aliases and Anthropic.
- Keep the same provider boundary as previous M7 work: `ProviderRegistry` constructs it from `ChatProviderConfig`, `ProviderManager` owns configured instances, runtime maps typed config, and pipeline only sees `ChatProvider`.
- Implement non-streaming text, system instruction, context role mapping, data URL image parts, HTTP error mapping, response text extraction, and safety/policy finish reason errors now.
- Return explicit provider errors for streaming and non-data URL image inputs until stream routing and media download/attachment conversion boundaries are designed.

## Verification

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
