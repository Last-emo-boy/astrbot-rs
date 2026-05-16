# M7-T2a Provider Parity: OpenAI-Compatible Aliases

## Reference

- AstrBot dynamically imports `openai_chat_completion`, `zhipu_chat_completion`, `groq_chat_completion`, `xai_chat_completion`, `aihubmix_chat_completion`, and `openrouter_chat_completion`.
- In AstrBot, Zhipu, Groq, xAI, AIHubMix, and OpenRouter subclass the OpenAI provider path, with small provider-specific request/header differences.
- OpenRouter injects `HTTP-Referer` and `X-TITLE`; AIHubMix injects `APP-Code`.

## Rust Decision

- Keep one `OpenAiCompatibleProvider` implementation and register AstrBot-compatible provider type aliases through `ProviderRegistry`.
- Expose `OPENAI_COMPATIBLE_CHAT_PROVIDER_TYPES` and per-provider type constants for config/UI discovery.
- Add `openai_compatible_with_type` constructors on `ChatProviderConfig` and `RuntimeChatProviderConfig` so runtime config can select an alias without duplicating provider logic.
- Implement only the default header parity for OpenRouter and AIHubMix in this step. Groq reasoning cleanup and xAI native search require request-shape fields that are not yet present in `ChatRequest`, so they remain deferred.

## Verification

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
