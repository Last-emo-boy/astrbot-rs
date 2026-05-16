# Runtime Handle Context

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/core_lifecycle.py`
  - `_load()` creates background tasks for event bus dispatch, platform adapters, cron and plugin tasks.
  - `stop()` cancels running tasks and terminates managers.
  - Task ownership stays in lifecycle instead of CLI.

## Rust Increment

- Added `PlatformManager::spawn_all()` so configured platform adapters can be started as task handles.
- Kept `PlatformManager::run_all()` as a convenience path for awaiting all adapter tasks.
- Added `RuntimeHandle` in `astrbot-runtime`.
- Added `AstrbotRuntime::start(self) -> RuntimeHandle`, which starts:
  - `EventBus::run()` as a background task
  - all configured platform adapter `run()` tasks
- `RuntimeHandle::stop()` aborts and joins event bus/platform tasks.
- Runtime handle preserves mock smoke helpers for integration tests:
  - `emit_mock_text()`
  - `emit_mock_text_on(platform_id, ...)`
  - `sent_messages()`
  - `sent_messages_for(platform_id)`

## Result

- Added a runtime test proving a started runtime handle can process a mock message through the background event bus and stop.
- Added a platform manager test covering `run_all()` for mock adapters.
- Verification passed:
  - `cargo fmt --all --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo run -p astrbot-cli`

## Next

- Add explicit reload semantics once config persistence and real platform adapters exist.
- Add graceful shutdown hooks for provider/plugin/platform managers when those managers own external resources.
