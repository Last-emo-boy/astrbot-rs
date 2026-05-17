# TASK-067 Summary

## Result

Added the `astrbot-backup` domain crate for backup manifest compatibility, import/export requests, repository ports, job progress snapshots, and upload session state. This mirrors AstrBot's backup concepts from `dashboard/routes/backup.py`, `core/backup/exporter.py`, and `core/backup/importer.py`, but keeps orchestration behind ports instead of embedding DB/file restore behavior inside HTTP routes.

Updated management backup routes so web handlers are DTO adapters over `BackupJobService` and `BackupUploadManager`. The route layer now exposes precheck, export, import, progress snapshot, upload start, chunk receipt, complete plan, and abort endpoints without owning background job maps.

## Files

- `Cargo.toml`
- `Cargo.lock`
- `crates/astrbot-backup/Cargo.toml`
- `crates/astrbot-backup/src/{lib,manifest,job,upload,import,export,service}.rs`
- `crates/astrbot-storage/{Cargo.toml,src/backup.rs,src/lib.rs}`
- `crates/astrbot-web/src/{lib.rs,management/mod.rs,management/backup.rs,tests/management.rs}`
- `.workflow/scratch/refactor-decoupling-audit-2026-05-16/{plan.json,index.json,.task/TASK-067.json,.summaries/TASK-067-summary.md}`

## Verification

- `cargo fmt --all`
- `cargo check --tests -p astrbot-backup`
- `cargo check --tests -p astrbot-storage`
- `cargo check --tests -p astrbot-web`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all --check`

`cargo test -p astrbot-backup`, `cargo test -p astrbot-storage`, and `cargo test -p astrbot-web` were attempted, but this shell cannot link test binaries because MSVC `link.exe` is missing. Test targets were still type-checked with `cargo check --tests`.
