# Plugin Stage Context

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/star/star_handler.py`
  - Handler metadata includes event type, plugin/module, handler name, filters, priority and enabled state.
  - Registry sorts handlers by descending priority.
- `E:/Playground/Astrbot/astrbot/core/star/filter/command.py`
  - Command filters match command names and aliases from message text.
- `E:/Playground/Astrbot/astrbot/core/pipeline/process_stage/method/star_request.py`
  - Activated plugin handlers run before LLM provider fallback.

## Rust Increment

- Add `astrbot-plugin` crate with `PluginRegistry`, `PluginHandler`, `EventFilter`, `CommandFilter`, `HandlerMetadata`.
- Add `PluginStage` before `ProviderStage`.
- Verify command plugin can produce a response and prevent provider fallback.

## Result

- Added `astrbot-plugin` crate.
- Implemented `PluginEventType`, `PluginControl`, `HandlerMetadata`, `EventFilter`, `CommandFilter`, `AlwaysFilter`, `PluginHandler`, `RegisteredHandler`, and `PluginRegistry`.
- `PluginRegistry` sorts handlers by descending priority, mirroring AstrBot's Star handler registry.
- Added `PluginStage` to `astrbot-pipeline`; it runs `AdapterMessage` plugin handlers before provider fallback.
- Added integration test verifying `/ping` command plugin replies `pong` and skips provider fallback.
- Verification passed: `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo run -p astrbot-cli`.
