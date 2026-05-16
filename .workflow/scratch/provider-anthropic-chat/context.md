# M7-T2b Provider Parity: Anthropic Chat

## Reference

- AstrBot registers `anthropic_chat_completion` through the provider registry.
- AstrBot's Anthropic provider separates system prompt from `messages`, sends requests to the Messages API, and reads text blocks from response `content`.
- AstrBot can convert local/base64 image data into Anthropic image blocks, while broader remote image download happens outside the provider request model.

## Rust Decision

- Add `AnthropicProvider` as the first non-OpenAI native chat provider.
- Keep the boundary identical to other providers: `ProviderRegistry` constructs it from `ChatProviderConfig`, `ProviderManager` owns instances, and pipeline only sees `ChatProvider`.
- Implement text, system prompt, context messages, error mapping, and data URL image blocks now.
- Return an explicit provider error for streaming and remote image URLs until stream routing and attachment/media download boundaries are designed.

## Verification

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
