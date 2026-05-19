use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::export::{BackupExportPackage, BackupExportRequest, checksum_bytes};
use crate::manifest::BackupManifest;
use crate::upload::{BackupUploadCompletePlan, secure_backup_filename};
use crate::{BackupExportPort, BackupTableDump};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilesystemBackupExporter {
    backup_root: PathBuf,
}

impl FilesystemBackupExporter {
    pub fn new(backup_root: impl Into<PathBuf>) -> Self {
        Self {
            backup_root: backup_root.into(),
        }
    }

    pub fn backup_root(&self) -> &Path {
        &self.backup_root
    }
}

#[async_trait]
impl BackupExportPort for FilesystemBackupExporter {
    async fn export_backup(&self, request: BackupExportRequest) -> Result<BackupExportPackage> {
        let filename = request
            .archive_filename
            .clone()
            .unwrap_or_else(|| archive_filename_from_export_time(&request.exported_at));
        let package = BackupExportPackage::from_request(request);
        write_backup_archive(package, &self.backup_root, &filename)
    }
}

pub fn write_backup_archive(
    mut package: BackupExportPackage,
    backup_root: impl AsRef<Path>,
    filename: &str,
) -> Result<BackupExportPackage> {
    let backup_root = backup_root.as_ref();
    fs::create_dir_all(backup_root).map_err(io_error("create backup root"))?;
    let filename = validated_backup_filename(filename)?;
    let output_path = backup_root.join(&filename);

    for entry in &package.generated_entries {
        validate_archive_path(&entry.archive_path)?;
        package
            .manifest
            .add_checksum(&entry.archive_path, checksum_bytes(&entry.bytes));
    }

    for entry in &package.files {
        validate_archive_path(&entry.archive_path)?;
        let checksum = checksum_file(Path::new(&entry.source_path))?;
        package.manifest.add_checksum(&entry.archive_path, checksum);
    }

    let result = (|| -> Result<()> {
        let file = File::create(&output_path).map_err(io_error("create backup archive"))?;
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        zip.start_file("manifest.json", options)
            .map_err(zip_error("start manifest"))?;
        let manifest_bytes = serde_json::to_vec_pretty(&package.manifest)
            .map_err(|err| AstrbotError::Pipeline(format!("serialize backup manifest: {err}")))?;
        zip.write_all(&manifest_bytes)
            .map_err(io_error("write manifest"))?;

        for entry in &package.generated_entries {
            zip.start_file(&entry.archive_path, options)
                .map_err(zip_error("start generated entry"))?;
            zip.write_all(&entry.bytes)
                .map_err(io_error("write generated entry"))?;
        }

        for entry in &package.files {
            zip.start_file(&entry.archive_path, options)
                .map_err(zip_error("start file entry"))?;
            let mut source =
                File::open(&entry.source_path).map_err(io_error("open backup source file"))?;
            std::io::copy(&mut source, &mut zip).map_err(io_error("copy backup source file"))?;
        }

        zip.finish().map_err(zip_error("finish backup archive"))?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = fs::remove_file(&output_path);
        return Err(error);
    }

    let size = fs::metadata(&output_path)
        .map_err(io_error("metadata backup archive"))?
        .len();
    Ok(package.with_archive(output_path, size))
}

pub fn read_backup_manifest(path: impl AsRef<Path>) -> Result<BackupManifest> {
    let path = path.as_ref();
    let mut archive = open_zip_archive(path)?;
    validate_archive_entries(&mut archive)?;
    let mut manifest_file = archive
        .by_name("manifest.json")
        .map_err(zip_error("read backup manifest"))?;
    let manifest: BackupManifest = read_json_from_zip_file(&mut manifest_file)?;
    Ok(manifest.with_source_path(path))
}

pub fn verify_backup_archive(path: impl AsRef<Path>) -> Result<BackupManifest> {
    let path = path.as_ref();
    let manifest = read_backup_manifest(path)?;
    let mut archive = open_zip_archive(path)?;
    for (entry_path, expected) in &manifest.checksums {
        validate_archive_path(entry_path)?;
        let mut file = archive
            .by_name(entry_path)
            .map_err(zip_error("read checksum entry"))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(io_error("read checksum entry"))?;
        if !checksum_matches(expected, &bytes) {
            return Err(AstrbotError::Pipeline(format!(
                "backup checksum mismatch for {entry_path}"
            )));
        }
    }
    Ok(manifest)
}

pub fn read_backup_table(
    path: impl AsRef<Path>,
    group: &str,
    table: &str,
) -> Result<BackupTableDump> {
    let entry_path = format!("databases/{group}/{table}.json");
    let rows = read_json_entry::<Vec<Value>>(path, &entry_path)?;
    Ok(BackupTableDump::new(group, table, rows))
}

pub fn read_entry_bytes(path: impl AsRef<Path>, entry_path: &str) -> Result<Vec<u8>> {
    validate_archive_path(entry_path)?;
    let mut archive = open_zip_archive(path.as_ref())?;
    let mut file = archive
        .by_name(entry_path)
        .map_err(zip_error("read backup entry"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(io_error("read backup entry"))?;
    Ok(bytes)
}

pub fn backup_archive_entry_names(path: impl AsRef<Path>) -> Result<Vec<String>> {
    let mut archive = open_zip_archive(path.as_ref())?;
    validate_archive_entries(&mut archive)?;
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(zip_error("read backup archive entry"))?;
        names.push(file.name().to_string());
    }
    Ok(names)
}

pub fn merge_upload_chunks(
    plan: &BackupUploadCompletePlan,
    backup_root: impl AsRef<Path>,
) -> Result<PathBuf> {
    let backup_root = backup_root.as_ref();
    fs::create_dir_all(backup_root).map_err(io_error("create backup root"))?;
    let filename = validated_backup_filename(&plan.filename)?;
    let output_path = backup_root.join(filename);

    let result = (|| -> Result<u64> {
        let mut output = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&output_path)
            .map_err(io_error("create merged backup upload"))?;
        let mut total = 0_u64;
        for index in &plan.ordered_chunk_indexes {
            let chunk_path = plan.chunk_dir.join(format!("{index}.part"));
            let mut chunk = File::open(&chunk_path).map_err(io_error("open backup chunk"))?;
            total +=
                std::io::copy(&mut chunk, &mut output).map_err(io_error("merge backup chunk"))?;
        }
        Ok(total)
    })();

    match result {
        Ok(total) if total == plan.total_size => {
            fs::remove_dir_all(&plan.chunk_dir).map_err(io_error("cleanup backup chunks"))?;
            Ok(output_path)
        }
        Ok(total) => {
            let _ = fs::remove_file(&output_path);
            Err(AstrbotError::Pipeline(format!(
                "merged backup upload has {total} bytes, expected {}",
                plan.total_size
            )))
        }
        Err(error) => {
            let _ = fs::remove_file(&output_path);
            Err(error)
        }
    }
}

fn read_json_entry<T: DeserializeOwned>(path: impl AsRef<Path>, entry_path: &str) -> Result<T> {
    validate_archive_path(entry_path)?;
    let mut archive = open_zip_archive(path.as_ref())?;
    let mut file = archive
        .by_name(entry_path)
        .map_err(zip_error("read json backup entry"))?;
    read_json_from_zip_file(&mut file)
}

fn read_json_from_zip_file<T: DeserializeOwned, R: Read>(reader: &mut R) -> Result<T> {
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(io_error("read json backup entry"))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| AstrbotError::Pipeline(format!("parse backup json entry: {err}")))
}

fn open_zip_archive(path: &Path) -> Result<ZipArchive<File>> {
    let file = File::open(path).map_err(io_error("open backup archive"))?;
    ZipArchive::new(file).map_err(zip_error("open backup archive"))
}

fn validate_archive_entries<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<()> {
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(zip_error("read backup archive entry"))?;
        validate_archive_path(file.name())?;
    }
    Ok(())
}

pub fn validate_archive_path(path: &str) -> Result<()> {
    if path.trim().is_empty() || path.contains('\\') || path.starts_with('/') {
        return Err(AstrbotError::Pipeline(format!(
            "backup archive path {path:?} is unsafe"
        )));
    }
    for component in Path::new(path).components() {
        match component {
            Component::Normal(_) => {}
            _ => {
                return Err(AstrbotError::Pipeline(format!(
                    "backup archive path {path:?} is unsafe"
                )));
            }
        }
    }
    Ok(())
}

fn checksum_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(io_error("open checksum source"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(io_error("read checksum source"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn checksum_matches(expected: &str, bytes: &[u8]) -> bool {
    if let Some(expected) = expected.strip_prefix("sha256:") {
        checksum_bytes(bytes).strip_prefix("sha256:") == Some(expected)
    } else if let Some(expected) = expected.strip_prefix("len:") {
        expected.parse::<usize>() == Ok(bytes.len())
    } else {
        false
    }
}

fn archive_filename_from_export_time(exported_at: &str) -> String {
    let stem = exported_at
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if stem.is_empty() {
        "astrbot-backup.zip".to_string()
    } else {
        format!("astrbot-backup-{stem}.zip")
    }
}

fn validated_backup_filename(filename: &str) -> Result<String> {
    let cleaned = secure_backup_filename(filename);
    if filename.trim().is_empty() || cleaned != filename.trim() {
        return Err(AstrbotError::Pipeline(
            "backup archive filename must be a safe direct filename".to_string(),
        ));
    }
    if cleaned.ends_with(".zip") {
        Ok(cleaned)
    } else {
        Ok(format!("{cleaned}.zip"))
    }
}

fn zip_error(context: &'static str) -> impl FnOnce(zip::result::ZipError) -> AstrbotError {
    move |err| AstrbotError::Pipeline(format!("zip {context}: {err}"))
}

fn io_error(context: &'static str) -> impl FnOnce(std::io::Error) -> AstrbotError {
    move |err| AstrbotError::Pipeline(format!("backup {context}: {err}"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::{
        FilesystemBackupExporter, merge_upload_chunks, read_backup_manifest, validate_archive_path,
        verify_backup_archive,
    };
    use crate::{
        BACKUP_UPLOAD_CHUNK_SIZE, BackupExportPort, BackupExportRequest, BackupTableDump,
        BackupUploadCompletePlan,
    };

    #[tokio::test]
    async fn filesystem_exporter_writes_zip_manifest_and_verifiable_checksums() {
        let root = temp_dir("archive-export");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root should exist");

        let exporter = FilesystemBackupExporter::new(&root);
        let package = exporter
            .export_backup(
                BackupExportRequest::new("4.9.1", "2026-05-18T00:00:00Z")
                    .with_archive_filename("fixture.zip")
                    .with_table_dump(BackupTableDump::new(
                        "main_db",
                        "conversations",
                        vec![json!({"id": "c1"})],
                    )),
            )
            .await
            .expect("export should write zip");

        let path = package.archive_path.expect("archive path");
        assert!(path.is_file());
        let manifest = verify_backup_archive(&path).expect("archive should verify");
        assert_eq!(
            manifest
                .table_statistics
                .get("main_db")
                .and_then(|tables| tables.get("conversations")),
            Some(&1)
        );
        assert!(
            manifest
                .checksums
                .get("databases/main_db/conversations.json")
                .is_some_and(|checksum| checksum.starts_with("sha256:"))
        );

        let loaded = read_backup_manifest(&path).expect("manifest should load");
        assert_eq!(loaded.source_path.as_deref(), Some(path.as_path()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_reader_rejects_path_traversal_entries() {
        assert!(validate_archive_path("files/config/app.json").is_ok());
        assert!(validate_archive_path("../evil").is_err());
        assert!(validate_archive_path("files\\evil").is_err());
        assert!(validate_archive_path("/absolute").is_err());
    }

    #[test]
    fn upload_complete_merges_chunks_and_cleans_chunk_dir() {
        let root = temp_dir("archive-upload");
        let chunk_dir = root.join("chunks/upload-1");
        let backup_root = root.join("backups");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&chunk_dir).expect("chunk dir");
        fs::write(
            chunk_dir.join("0.part"),
            vec![b'a'; BACKUP_UPLOAD_CHUNK_SIZE as usize],
        )
        .expect("first chunk");
        fs::write(chunk_dir.join("1.part"), b"tail").expect("second chunk");

        let output = merge_upload_chunks(
            &BackupUploadCompletePlan {
                upload_id: "upload-1".to_string(),
                filename: "merged.zip".to_string(),
                chunk_dir: chunk_dir.clone(),
                ordered_chunk_indexes: vec![0, 1],
                total_size: BACKUP_UPLOAD_CHUNK_SIZE + 4,
            },
            &backup_root,
        )
        .expect("chunks should merge");

        assert_eq!(
            fs::metadata(&output).expect("merged file").len(),
            BACKUP_UPLOAD_CHUNK_SIZE + 4
        );
        assert!(!chunk_dir.exists());

        let _ = fs::remove_dir_all(root);
    }

    fn temp_dir(suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("astrbot-backup-{suffix}-{}", std::process::id()))
    }
}
