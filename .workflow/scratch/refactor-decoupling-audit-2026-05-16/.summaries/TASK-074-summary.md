# TASK-074 Summary

Completed at: 2026-05-17T13:28:58+08:00

## Scope

Split the storage schema catalog into schema primitives, domain table-family builders, and migration seed consumption boundaries.

## AstrBot Reference

- `E:/Playground/Astrbot/astrbot/core/db/po.py`
- `E:/Playground/Astrbot/astrbot/core/db/sqlite.py`
- `E:/Playground/Astrbot/astrbot/core/db/migration`

## Changes

- Replaced the flat `schema.rs` catalog with a `schema/` module tree while keeping `crate::schema::{StorageSchema, StorageTable, StorageColumn, StorageColumnType}` and crate-root re-exports stable.
- Moved schema primitives into `schema/table.rs`.
- Split table-family builders into `conversation`, `platform`, `provider`, `persona_skill`, and `ops` modules.
- Kept `repository_port_schema()` and `astrbot_main_v4()` as facade constructors that assemble domain-owned table descriptors.
- Added `DeclarativeMigration::create_schema` so migration seeds consume schema descriptors instead of owning table construction.
- Added tests for public schema identity, stable table ordering, and schema-backed migration seed operations.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-storage`
- `cargo clippy --workspace -- -D warnings`

## Next

Next pending task is `TASK-075`.
