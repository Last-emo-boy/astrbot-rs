# Runtime Config Normalization Context

## Reference From AstrBot

- `E:/Playground/Astrbot/astrbot/core/config/astrbot_config.py`
  - `AstrBotConfig.__init__()` creates a missing config file from defaults.
  - `check_config_integrity()` inserts missing default keys and persists the repaired config.
  - This keeps runtime config files self-healing across version changes.

## Rust Increment

- `RuntimeConfig::from_json_file()` now:
  - creates missing files from `RuntimeConfig::default()`
  - reads existing JSON into typed config
  - detects missing default keys by comparing JSON shape to `RuntimeConfig::default()`
  - writes the normalized typed config back when defaults were missing
- Added helpers:
  - `write_runtime_config()`
  - `config_needs_default_merge()`
  - `json_missing_default_keys()`

## Result

- Existing missing-file test now also checks the persisted file contains runtime defaults.
- Added `missing_top_level_defaults_are_written_back` to verify partial config files are repaired.
- Verification passed:
  - `cargo fmt --all --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo run -p astrbot-cli`

## Next

- Add schema/version fields when runtime config grows beyond the current typed MVP.
- Decide whether unknown keys should be removed like AstrBot or preserved for forward compatibility.
