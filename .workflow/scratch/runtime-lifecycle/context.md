# Runtime Lifecycle Context

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/core_lifecycle.py`
  - `AstrBotCoreLifecycle.initialize()` centralizes component assembly: config manager, provider manager, platform manager, plugin manager, pipeline scheduler mapping and event bus.
  - `load_pipeline_scheduler()` creates schedulers from config and plugin context instead of letting CLI or platform code assemble stages.
  - `_load()` owns long-running tasks such as event bus dispatch, cron and plugin tasks.
- `E:/Playground/Astrbot/astrbot/core/config/astrbot_config.py`
  - Missing config files are created from defaults.
  - Existing config is reconciled with default structure.
- `E:/Playground/Astrbot/astrbot/core/config/default.py`
  - Provider defaults include `default_provider_id`, provider enable switches and plugin selection knobs.

## Rust Increment

- Add `astrbot-runtime` as the lifecycle composition crate.
- Move CLI smoke wiring into `AstrbotRuntime::initialize(RuntimeConfig)`.
- Keep `EventBus` thin and keep business behavior in plugin/provider/pipeline crates.
- Runtime builds:
  - `ProviderManager` through `ProviderRegistry`
  - `PluginRegistry` from typed command plugin config
  - `PipelineScheduler` with `PluginStage -> ProviderStage -> RespondStage`
  - `MockPlatform` and event queue for the current smoke platform
- Runtime stores manager/registry/scheduler fields for future dashboard/reload use.
- Runtime allows plugin-only responses when no chat provider is configured, so provider remains a fallback rather than a hard dependency.

## Result

- `crates/astrbot-runtime/src/lib.rs` now provides `RuntimeConfig`, `RuntimeChatProviderConfig`, `RuntimeCommandPluginConfig` and `AstrbotRuntime`.
- `crates/astrbot-cli/src/main.rs` now delegates lifecycle wiring to `astrbot-runtime`.
- Verification passed:
  - `cargo fmt --all --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo run -p astrbot-cli`

## Next

- Add platform registry/manager so runtime can construct real adapters from config instead of only `MockPlatform`.
- Add reload/start-stop task management mirroring `AstrBotCoreLifecycle.start()` and `stop()`.
- Add config integrity merge behavior closer to `AstrBotConfig.check_config_integrity()`.
