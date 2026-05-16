# TASK-032 Summary - Persistence Migration Boundary

## Outcome

Extended `astrbot-storage` with typed persistence, migration, stats, backup, repository, and SQLite planning boundaries. This keeps storage ports independent from concrete SQLite wiring and keeps backup import/export out of dashboard route handlers.

New modules:

- `schema.rs`: typed storage schema/table/column model, including AstrBot main DB v4 table boundaries.
- `migration.rs`: migration operation DTOs, migration state repository, in-memory state, declarative migrations, and idempotent runner.
- `stats.rs`: platform stats repository port and in-memory merge-by-hour/platform/type implementation.
- `repository.rs`: repository implementation descriptors and backend identity boundary.
- `sqlite.rs`: SQLite config/pragma/schema plan, preserving AstrBot's WAL/NORMAL/cache/temp/mmap pragma shape.
- `backup/manifest.rs`: backup manifest, schema version, directory stats, checksums, and version-status DTOs.
- `backup/export.rs`: export request/package/table dump/file entry boundary that builds manifest data without route handlers.
- `backup/import.rs`: import precheck, import mode/result, version compatibility policy, and import port.

## AstrBot Reference

Compared against:

- `E:/Playground/Astrbot/astrbot/core/db/__init__.py`
- `E:/Playground/Astrbot/astrbot/core/db/sqlite.py`
- `E:/Playground/Astrbot/astrbot/core/db/po.py`
- `E:/Playground/Astrbot/astrbot/core/db/migration/helper.py`
- `E:/Playground/Astrbot/astrbot/core/db/migration/migra_token_usage.py`
- `E:/Playground/Astrbot/astrbot/core/backup/constants.py`
- `E:/Playground/Astrbot/astrbot/core/backup/exporter.py`
- `E:/Playground/Astrbot/astrbot/core/backup/importer.py`

Rust keeps AstrBot's DB schema, migration markers, platform stats upsert behavior, backup manifest, version precheck, and import/export result concepts, but expresses them as typed Rust ports and plans before connecting a real SQLite backend.

## Verification

- `cargo fmt --all`
- `cargo test -p astrbot-storage`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
