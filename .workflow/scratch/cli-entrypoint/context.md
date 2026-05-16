# CLI Entrypoint Context

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/core_lifecycle.py`
  - CLI/bootstrap should create or load config and then delegate to lifecycle/runtime.
  - Long-running task ownership belongs to lifecycle/runtime, not command parsing code.

## Rust Increment

- `astrbot-cli` now supports:
  - `smoke [config_path]`: run one mock message through the runtime
  - `init [config_path]`: create or normalize a runtime config file
  - `run [config_path]`: start runtime tasks and wait for `Ctrl+C`
- Default no-arg behavior remains smoke mode so `cargo run -p astrbot-cli` stays a fast sanity check.
- Added manual parser tests for default, `init` and `run` command paths.
- Added Tokio `signal` feature so CLI can await `Ctrl+C`.

## Result

- `cargo run -p astrbot-cli` still prints the smoke response.
- `cargo run -p astrbot-cli -- init target/astrbot-cli-init-check.json` creates a config file successfully.
- Verification passed:
  - `cargo fmt --all --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo run -p astrbot-cli`

## Next

- Add richer command parsing once the CLI grows beyond this small command set.
- Wire real platform adapters so `run` does useful work without mock helpers.
