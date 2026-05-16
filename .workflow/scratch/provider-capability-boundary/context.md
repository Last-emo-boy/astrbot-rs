# M7-T2d Provider Parity: Capability Boundary

## Reference

- AstrBot defines `ProviderType` with `chat_completion`, `speech_to_text`, `text_to_speech`, `embedding`, and `rerank`.
- AstrBot provider adapter registration stores provider metadata separately from configured instances.
- AstrBot `ProviderManager` keeps separate buckets for chat, STT, TTS, embedding, and rerank providers, while `inst_map` remains keyed by configured provider ID.

## Rust Decision

- Add `ProviderCapability` and `ProviderAdapterMetadata` to `astrbot-provider`.
- `ProviderRegistry::register_chat_provider()` now also registers adapter metadata with `ProviderCapability::ChatCompletion`.
- `ProviderRegistry::register_provider_adapter()` can register future non-chat metadata before concrete STT/TTS/Embedding/Rerank factories exist.
- `ProviderManager::from_chat_configs()` remains chat-only, and registry errors clearly when non-chat metadata is used as a chat provider type.

## Deferred

- Concrete STT/TTS/Embedding/Rerank traits, configs, factories, managers, runtime config, and dashboard APIs.
- Session/global provider preference for non-chat capabilities.

## Verification

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo run -p astrbot-cli`
