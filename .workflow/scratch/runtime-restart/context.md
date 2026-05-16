# Runtime Restart Context

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/core_lifecycle.py`
  - `restart()` terminates provider/platform state and triggers a rebuild path.
  - Restart is modeled as a lifecycle transition, not as an ad hoc CLI concern.

## Rust Increment

- Added `RuntimeHandle::restart(self, config)` to `astrbot-runtime`.
- Restart path:
  1. stop the current runtime handle
  2. construct a fresh `AstrbotRuntime` from the new config
  3. start a new `RuntimeHandle`
- Added builder helpers on `RuntimeConfig` to make restart tests and future callers cleaner:
  - `RuntimeConfig::new(...)`
  - `RuntimeConfig::with_default_chat_provider_id(...)`

## Result

- Added a restart test that:
  - starts a runtime
  - processes a mock event
  - restarts with a new mock provider response
  - processes another event through the restarted runtime
- Verification passed:
  - `cargo fmt --all --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo run -p astrbot-cli`

## Next

- Decide whether runtime reload should preserve session state or rebuild everything from scratch.
- Add graceful lifecycle hooks for future non-mock platform/provider resources.
