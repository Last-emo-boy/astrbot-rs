# Provider Registry Context

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/provider/register.py`
  - `provider_cls_map` maps provider type names to metadata and classes.
- `E:/Playground/Astrbot/astrbot/core/provider/manager.py`
  - `inst_map` maps configured provider IDs to instantiated providers.
  - `get_using_provider` resolves session/default provider selection.
  - Missing configured default falls back to first available provider.

## Rust Increment

- Add `ProviderRegistry` for provider-type factory registration.
- Add `ProviderManager` for configured provider instances and default chat selection.
- Add typed `ChatProviderConfig` instead of raw dynamic dicts.
- Keep session-specific provider preference deferred until storage/session layer exists.

## Result

- Added `ProviderRegistry` and `ProviderManager` in `astrbot-provider`.
- Added typed `ChatProviderConfig` with redacted `Debug` output for API keys and mock responses.
- Registered built-in `mock_chat_completion` and `openai_chat_completion` factories.
- Manager now builds enabled provider configs, stores instances by configured ID, selects configured default, and falls back to first provider when the default is missing.
- CLI now obtains its provider through `ProviderRegistry` + `ProviderManager` instead of constructing concrete providers directly.
- Verification passed: `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo run -p astrbot-cli`.
