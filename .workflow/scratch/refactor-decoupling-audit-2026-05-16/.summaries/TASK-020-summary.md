# TASK-020 Summary - Config Schema Boundary

Status: completed

## Scope

Split runtime config defaults, schema metadata, env resolution, secret redaction, and migration policy so dashboard/config work does not grow inside `RuntimeConfig` or config IO.

## Changes

- Added `config/defaults.rs` and kept root `defaults.rs` as a compatibility facade.
- Added `config/env.rs` with `RuntimeEnvConfigSource` and testable lookup-based env loading.
- Added `config/secrets.rs` with `SecretValue`, `REDACTED_SECRET`, and optional secret redaction helpers.
- Added `config/migration.rs` with `RuntimeConfigMigrationPlan` and default-key merge detection.
- Added `config/schema.rs` with typed runtime config schema fields and secret marking.
- Added `config/ui_metadata.rs` with dashboard-facing UI groups and controls.
- Kept `RuntimeConfig::from_json_file` and `RuntimeConfig::from_env` as stable public entry points.
- Re-exported schema/env/secret/UI metadata types from `astrbot-runtime`.
- Added focused tests in `tests/config_schema.rs`.

## AstrBot Reference

This follows the separation pressure visible in `E:/Playground/Astrbot/astrbot/core/config/default.py` and `config/astrbot_config.py`: defaults, schema-derived defaults, UI metadata, integrity checking, and secret fields should not live in one growing config file.

## Verification

- `cargo fmt --all --check`
- `cargo test -p astrbot-runtime`
- `cargo clippy -p astrbot-runtime -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
