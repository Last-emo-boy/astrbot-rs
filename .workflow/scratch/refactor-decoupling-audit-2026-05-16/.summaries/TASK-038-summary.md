# TASK-038 Summary

Completed: 2026-05-16T21:19:56+08:00

Scope:
- Define a runtime-owned path policy matching AstrBot's `data/*` directory layout.
- Define a storage-owned temporary artifact lifecycle and cleanup policy.
- Move provider generated media defaults away from ad hoc `std::env::temp_dir` policy.

Changes:
- Added `crates/astrbot-runtime/src/path_config.rs` with `RuntimePathConfig` and `RuntimePathLayout`.
- Added `RuntimeConfig.paths`, schema entries, UI metadata, and path layout tests.
- Added `crates/astrbot-storage/src/temp_artifact.rs` with `TempArtifactRoot`, `TempArtifactDescriptor`, cleanup policy, cleanup plan, and cleaner.
- Re-exported temp artifact types from `astrbot-storage`.
- Updated provider generated media default TTS output to derive from `TempArtifactRoot::default()/generated_media/tts`.

AstrBot reference:
- `core/utils/astrbot_path.py` centralizes root, data, config, plugin, plugin_data, temp, skills, site-packages, knowledge_base, backups, WebChat, and T2I template paths.
- `core/utils/temp_dir_cleaner.py` scans temp files, applies max-size cleanup, removes oldest files first, and cleans empty directories.
- `core/backup/exporter.py` treats data subdirectories and temp artifacts as managed backup/export surfaces.

Verification:
- `cargo fmt --all --check`
- `cargo test -p astrbot-storage`
- `cargo test -p astrbot-runtime`
- `cargo test -p astrbot-provider`
- `cargo clippy -p astrbot-storage -- -D warnings`
- `cargo clippy -p astrbot-runtime -- -D warnings`
- `cargo clippy -p astrbot-provider -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

Next:
- `M7-R34-t2i-render-boundary` / `TASK-039`.
