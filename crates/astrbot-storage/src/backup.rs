use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub use astrbot_backup::{
    BACKUP_MANIFEST_VERSION, BACKUP_UPLOAD_CHUNK_SIZE, BACKUP_UPLOAD_EXPIRE_SECONDS,
    BackupArchiveEntry, BackupChunkReceipt, BackupDirectoryStat, BackupExportJobRequest,
    BackupExportPackage, BackupExportPort, BackupExportRequest, BackupFileEntry,
    BackupImportJobRequest, BackupImportMode, BackupImportPort, BackupImportPrecheck,
    BackupImportResult, BackupJobKind, BackupJobService, BackupJobSnapshot, BackupJobStatus,
    BackupJobStore, BackupManifest, BackupProgressReader, BackupProgressSnapshot,
    BackupRepositoryPort, BackupSchemaVersion, BackupTableDump, BackupUploadCompletePlan,
    BackupUploadManager, BackupUploadSession, BackupUploadStart, BackupVersionStatus,
    FilesystemBackupExporter, backup_archive_entry_names, merge_upload_chunks,
    read_backup_manifest, read_backup_table, read_entry_bytes, secure_backup_filename,
    validate_archive_path, verify_backup_archive,
};
use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, params_from_iter};
use serde_json::{Map, Value};

use crate::SqliteStorage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteBackupDirectory {
    pub name: String,
    pub path: PathBuf,
}

impl SqliteBackupDirectory {
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteBackupRepository {
    sqlite_path: PathBuf,
    backup_root: PathBuf,
    directories: Vec<SqliteBackupDirectory>,
}

impl SqliteBackupRepository {
    pub fn new(sqlite_path: impl Into<PathBuf>, backup_root: impl Into<PathBuf>) -> Self {
        Self {
            sqlite_path: sqlite_path.into(),
            backup_root: backup_root.into(),
            directories: Vec::new(),
        }
    }

    pub fn with_directory(mut self, name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        self.directories
            .push(SqliteBackupDirectory::new(name, path));
        self
    }

    pub fn backup_root(&self) -> &Path {
        &self.backup_root
    }
}

#[async_trait]
impl BackupRepositoryPort for SqliteBackupRepository {
    async fn collect_export(
        &self,
        request: &BackupExportJobRequest,
    ) -> Result<BackupExportRequest> {
        let mut export = BackupExportRequest::new(&request.astrbot_version, &request.exported_at)
            .with_archive_filename(export_filename(&request.task_id, &request.exported_at));

        if self.sqlite_path.is_file() {
            for dump in dump_sqlite_tables(&self.sqlite_path)? {
                export = export.with_table_dump(dump);
            }
        }

        for directory in &self.directories {
            let (files, stat) = collect_directory_files(&directory.name, &directory.path)?;
            for file in files {
                export = export.with_file(file);
            }
            export = export.with_directory(&directory.name, stat);
        }

        Ok(export)
    }

    async fn load_import_manifest(
        &self,
        request: &BackupImportJobRequest,
    ) -> Result<BackupManifest> {
        let path = resolve_backup_file(&self.backup_root, &request.source_id)?;
        read_backup_manifest(path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteBackupImporter {
    current_version: String,
    sqlite_path: PathBuf,
    directories: BTreeMap<String, PathBuf>,
}

impl SqliteBackupImporter {
    pub fn new(current_version: impl Into<String>, sqlite_path: impl Into<PathBuf>) -> Self {
        Self {
            current_version: current_version.into(),
            sqlite_path: sqlite_path.into(),
            directories: BTreeMap::new(),
        }
    }

    pub fn with_directory(mut self, name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        self.directories.insert(name.into(), path.into());
        self
    }
}

#[async_trait]
impl BackupImportPort for SqliteBackupImporter {
    async fn precheck_backup(&self, manifest: &BackupManifest) -> Result<BackupImportPrecheck> {
        if let Some(source_path) = &manifest.source_path {
            verify_backup_archive(source_path)?;
        }
        Ok(BackupImportPrecheck::from_manifest(
            manifest,
            self.current_version.clone(),
        ))
    }

    async fn import_backup(
        &self,
        manifest: BackupManifest,
        mode: BackupImportMode,
    ) -> Result<BackupImportResult> {
        let source_path = manifest.source_path.clone().ok_or_else(|| {
            AstrbotError::Pipeline("backup import source path is missing".to_string())
        })?;
        let manifest = verify_backup_archive(&source_path)?;
        let mut result = BackupImportResult::success();

        restore_sqlite_tables(
            &self.sqlite_path,
            &source_path,
            &manifest,
            &mode,
            &mut result,
        )?;
        restore_directories(
            &source_path,
            &manifest,
            &self.directories,
            &mode,
            &mut result,
        )?;

        Ok(result)
    }
}

fn dump_sqlite_tables(path: &Path) -> Result<Vec<BackupTableDump>> {
    let conn = Connection::open(path).map_err(sqlite_error("open backup source"))?;
    let mut dumps = Vec::new();
    for table in sqlite_table_names(&conn)? {
        dumps.push(dump_sqlite_table(&conn, &table)?);
    }
    Ok(dumps)
}

fn sqlite_table_names(conn: &Connection) -> Result<Vec<String>> {
    let mut statement = conn
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .map_err(sqlite_error("prepare table list"))?;
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sqlite_error("query table list"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sqlite_error("collect table list"))
}

fn dump_sqlite_table(conn: &Connection, table: &str) -> Result<BackupTableDump> {
    let sql = format!("SELECT * FROM {}", quote_identifier(table));
    let mut statement = conn
        .prepare(&sql)
        .map_err(sqlite_error("prepare table dump"))?;
    let columns = statement
        .column_names()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query table dump"))?;
    let mut values = Vec::new();
    while let Some(row) = rows.next().map_err(sqlite_error("read table row"))? {
        let mut object = Map::new();
        for (index, column) in columns.iter().enumerate() {
            object.insert(
                column.clone(),
                sqlite_value_to_json(row.get_ref(index).map_err(sqlite_error("read column"))?),
            );
        }
        values.push(Value::Object(object));
    }
    Ok(BackupTableDump::new("main_db", table, values))
}

fn collect_directory_files(
    name: &str,
    root: &Path,
) -> Result<(Vec<BackupFileEntry>, BackupDirectoryStat)> {
    let mut files = Vec::new();
    let mut stat = BackupDirectoryStat::default();
    if !root.exists() {
        return Ok((files, stat));
    }
    collect_directory_files_inner(name, root, root, &mut files, &mut stat)?;
    Ok((files, stat))
}

fn collect_directory_files_inner(
    name: &str,
    root: &Path,
    current: &Path,
    files: &mut Vec<BackupFileEntry>,
    stat: &mut BackupDirectoryStat,
) -> Result<()> {
    for entry in fs::read_dir(current).map_err(io_error("read backup directory"))? {
        let entry = entry.map_err(io_error("read backup directory entry"))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(io_error("read backup file metadata"))?;
        if metadata.is_dir() {
            collect_directory_files_inner(name, root, &path, files, stat)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|err| AstrbotError::Pipeline(format!("backup relative path: {err}")))?;
            let relative = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("/");
            let archive_path = format!("files/{name}/{relative}");
            validate_archive_path(&archive_path)?;
            files.push(BackupFileEntry::new(archive_path, &path, metadata.len()));
            stat.files += 1;
            stat.size_bytes += metadata.len();
        }
    }
    Ok(())
}

fn restore_sqlite_tables(
    sqlite_path: &Path,
    source_path: &Path,
    manifest: &BackupManifest,
    mode: &BackupImportMode,
    result: &mut BackupImportResult,
) -> Result<()> {
    if let Some(parent) = sqlite_path.parent() {
        fs::create_dir_all(parent).map_err(io_error("create sqlite directory"))?;
    }
    let _ = SqliteStorage::open(sqlite_path)?;
    let conn = Connection::open(sqlite_path).map_err(sqlite_error("open restore target"))?;
    conn.execute_batch("PRAGMA foreign_keys = OFF")
        .map_err(sqlite_error("disable foreign keys"))?;
    let mut replaced_tables = BTreeSet::new();

    for (group, tables) in &manifest.tables {
        for table in tables {
            if matches!(mode, BackupImportMode::Replace) && replaced_tables.insert(table.clone()) {
                conn.execute(&format!("DELETE FROM {}", quote_identifier(table)), [])
                    .map_err(sqlite_error("clear restore table"))?;
            }
            let dump = read_backup_table(source_path, group, table)?;
            let mut imported = 0_usize;
            for row in &dump.rows {
                let Some(object) = row.as_object() else {
                    result.add_warning(format!("skipped non-object row in table {table}"));
                    continue;
                };
                if object.is_empty() {
                    continue;
                }
                insert_sqlite_row(&conn, table, object)?;
                imported += 1;
            }
            result
                .imported_tables
                .insert(format!("{group}.{table}"), imported);
        }
    }

    Ok(())
}

fn insert_sqlite_row(conn: &Connection, table: &str, object: &Map<String, Value>) -> Result<()> {
    let columns = object.keys().cloned().collect::<Vec<_>>();
    let placeholders = (0..columns.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let column_sql = columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT OR REPLACE INTO {} ({column_sql}) VALUES ({placeholders})",
        quote_identifier(table)
    );
    let values = columns
        .iter()
        .map(|column| json_to_sql_value(&object[column]))
        .collect::<Vec<_>>();
    conn.execute(&sql, params_from_iter(values))
        .map_err(sqlite_error("insert restore row"))?;
    Ok(())
}

fn restore_directories(
    source_path: &Path,
    manifest: &BackupManifest,
    directories: &BTreeMap<String, PathBuf>,
    mode: &BackupImportMode,
    result: &mut BackupImportResult,
) -> Result<()> {
    let entry_names = backup_archive_entry_names(source_path)?;
    for directory in &manifest.directories {
        let Some(target_root) = directories.get(directory) else {
            result.add_warning(format!(
                "backup directory {directory} has no restore target"
            ));
            continue;
        };
        if matches!(mode, BackupImportMode::Replace) && target_root.exists() {
            clear_directory(target_root)?;
        }
        fs::create_dir_all(target_root).map_err(io_error("create restore directory"))?;

        let prefix = format!("files/{directory}/");
        let mut restored = 0_usize;
        for entry in entry_names
            .iter()
            .filter(|entry| entry.starts_with(&prefix) && !entry.ends_with('/'))
        {
            let relative = &entry[prefix.len()..];
            let target = safe_join(target_root, relative)?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(io_error("create restore file parent"))?;
            }
            let bytes = read_entry_bytes(source_path, entry)?;
            fs::write(target, bytes).map_err(io_error("write restore file"))?;
            restored += 1;
        }
        result
            .imported_directories
            .insert(directory.clone(), restored);
    }
    Ok(())
}

fn clear_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(io_error("create clear directory"))?;
    for entry in fs::read_dir(path).map_err(io_error("read clear directory"))? {
        let entry = entry.map_err(io_error("read clear directory entry"))?;
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(path).map_err(io_error("remove restore directory"))?;
        } else {
            fs::remove_file(path).map_err(io_error("remove restore file"))?;
        }
    }
    Ok(())
}

fn resolve_backup_file(root: &Path, source_id: &str) -> Result<PathBuf> {
    let filename = validate_backup_filename(source_id)?;
    let path = root.join(filename);
    if path.parent() != Some(root) {
        return Err(AstrbotError::Pipeline(
            "backup source must stay within backup root".to_string(),
        ));
    }
    if !path.is_file() {
        return Err(AstrbotError::Pipeline(format!(
            "backup file {} was not found",
            source_id
        )));
    }
    Ok(path)
}

fn safe_join(root: &Path, relative: &str) -> Result<PathBuf> {
    validate_archive_path(relative)?;
    let target = root.join(relative);
    let parent = target
        .parent()
        .ok_or_else(|| AstrbotError::Pipeline("restore target has no parent".to_string()))?;
    if !parent.starts_with(root) {
        return Err(AstrbotError::Pipeline(
            "restore target escapes directory".to_string(),
        ));
    }
    Ok(target)
}

fn validate_backup_filename(filename: &str) -> Result<String> {
    let trimmed = filename.trim();
    let safe = secure_backup_filename(trimmed);
    if trimmed.is_empty() || safe != trimmed {
        return Err(AstrbotError::Pipeline(
            "backup filename must be a safe direct filename".to_string(),
        ));
    }
    Ok(safe)
}

fn export_filename(task_id: &str, exported_at: &str) -> String {
    let stem = if task_id.trim().is_empty() {
        exported_at
    } else {
        task_id
    };
    let cleaned = stem
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if cleaned.is_empty() {
        "astrbot-backup.zip".to_string()
    } else {
        format!("{cleaned}.zip")
    }
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn sqlite_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => Value::from(value),
        ValueRef::Real(value) => Value::from(value),
        ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).to_string()),
        ValueRef::Blob(value) => Value::Array(
            value
                .iter()
                .map(|byte| Value::from(u64::from(*byte)))
                .collect(),
        ),
    }
}

fn json_to_sql_value(value: &Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(value) => SqlValue::Integer(i64::from(*value)),
        Value::Number(value) => value
            .as_i64()
            .map(SqlValue::Integer)
            .or_else(|| value.as_f64().map(SqlValue::Real))
            .unwrap_or(SqlValue::Null),
        Value::String(value) => SqlValue::Text(value.clone()),
        Value::Array(values) => {
            let bytes = values
                .iter()
                .map(|value| value.as_u64().and_then(|value| u8::try_from(value).ok()))
                .collect::<Option<Vec<_>>>();
            match bytes {
                Some(bytes) => SqlValue::Blob(bytes),
                None => SqlValue::Text(Value::Array(values.clone()).to_string()),
            }
        }
        Value::Object(value) => SqlValue::Text(Value::Object(value.clone()).to_string()),
    }
}

fn sqlite_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> AstrbotError {
    move |err| AstrbotError::Pipeline(format!("sqlite backup {context}: {err}"))
}

fn io_error(context: &'static str) -> impl FnOnce(std::io::Error) -> AstrbotError {
    move |err| AstrbotError::Pipeline(format!("backup {context}: {err}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{FilesystemBackupExporter, SqliteBackupImporter, SqliteBackupRepository};
    use crate::{
        ApiKeyRecord, ApiKeyRepository, BackupExportJobRequest, BackupImportJobRequest,
        BackupImportMode, BackupJobService, BackupJobStatus, BackupRepositoryPort, SqliteStorage,
        verify_backup_archive,
    };

    #[tokio::test]
    async fn sqlite_backup_exports_imports_and_restores_files() {
        let root = temp_dir("sqlite-roundtrip");
        let source_db = root.join("source/main.sqlite");
        let target_db = root.join("target/main.sqlite");
        let backup_root = root.join("backups");
        let source_config = root.join("source/config");
        let target_config = root.join("target/config");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&source_config).expect("source config");
        std::fs::write(
            source_config.join("cmd_config.json"),
            br#"{"enabled":true}"#,
        )
        .expect("config fixture");

        let source = SqliteStorage::open(&source_db).expect("source sqlite");
        source
            .store_api_key(ApiKeyRecord::new(
                "key-1",
                "Dashboard",
                "hash-1",
                "ak_",
                ["management.read"],
                "admin",
            ))
            .await
            .expect("seed source");

        let repository = Arc::new(
            SqliteBackupRepository::new(&source_db, &backup_root)
                .with_directory("config", &source_config),
        );
        let service = BackupJobService::new(
            repository.clone(),
            Arc::new(FilesystemBackupExporter::new(&backup_root)),
            Arc::new(
                SqliteBackupImporter::new("4.9.1", &target_db)
                    .with_directory("config", &target_config),
            ),
        );

        let export = service
            .start_export(BackupExportJobRequest::new(
                "export-roundtrip",
                "4.9.1",
                "2026-05-18T00:00:00Z",
            ))
            .await
            .expect("export should complete");
        assert_eq!(export.progress.status, BackupJobStatus::Completed);
        let backup_file = backup_root.join("export-roundtrip.zip");
        let manifest = verify_backup_archive(&backup_file).expect("backup should verify");
        assert!(
            manifest
                .checksums
                .get("databases/main_db/api_keys.json")
                .is_some_and(|checksum| checksum.starts_with("sha256:"))
        );

        let precheck = service
            .precheck_import(&BackupImportJobRequest::new(
                "precheck-roundtrip",
                "export-roundtrip.zip",
                BackupImportMode::Replace,
            ))
            .await
            .expect("precheck should read archive manifest");
        assert!(precheck.can_import);

        service
            .start_import(BackupImportJobRequest::new(
                "import-roundtrip",
                "export-roundtrip.zip",
                BackupImportMode::Replace,
            ))
            .await
            .expect("import should complete");

        let restored = SqliteStorage::open(&target_db).expect("target sqlite");
        assert!(
            restored
                .api_key_by_hash("hash-1")
                .await
                .expect("api key query")
                .is_some()
        );
        assert_eq!(
            std::fs::read_to_string(target_config.join("cmd_config.json"))
                .expect("restored config"),
            r#"{"enabled":true}"#
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn sqlite_backup_records_failed_progress_for_bad_zip() {
        let root = temp_dir("sqlite-bad-zip");
        let backup_root = root.join("backups");
        let target_db = root.join("target.sqlite");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&backup_root).expect("backup root");
        std::fs::write(backup_root.join("bad.zip"), b"not-a-zip").expect("bad zip");

        let repository = Arc::new(SqliteBackupRepository::new(
            root.join("source.sqlite"),
            &backup_root,
        ));
        let service = BackupJobService::new(
            repository,
            Arc::new(FilesystemBackupExporter::new(&backup_root)),
            Arc::new(SqliteBackupImporter::new("4.9.1", &target_db)),
        );

        let error = service
            .start_import(BackupImportJobRequest::new(
                "import-bad",
                "bad.zip",
                BackupImportMode::Replace,
            ))
            .await
            .expect_err("bad zip should fail");
        assert!(error.to_string().contains("zip"));
        let snapshot = service
            .jobs()
            .snapshot("import-bad")
            .expect("snapshot")
            .expect("failed snapshot");
        assert_eq!(snapshot.progress.status, BackupJobStatus::Failed);

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn sqlite_backup_rejects_traversal_source_id() {
        let root = temp_dir("sqlite-traversal");
        let repository =
            SqliteBackupRepository::new(root.join("source.sqlite"), root.join("backups"));
        let error = repository
            .load_import_manifest(&BackupImportJobRequest::new(
                "precheck-traversal",
                "../evil.zip",
                BackupImportMode::Replace,
            ))
            .await
            .expect_err("traversal should fail");
        assert!(error.to_string().contains("safe direct filename"));

        let _ = std::fs::remove_dir_all(root);
    }

    fn temp_dir(suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "astrbot-storage-backup-{suffix}-{}",
            std::process::id()
        ))
    }
}
