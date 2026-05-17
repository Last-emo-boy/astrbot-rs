use std::collections::BTreeMap;

use astrbot_core::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::manifest::{BackupDirectoryStat, BackupManifest};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackupTableDump {
    pub group: String,
    pub table: String,
    pub rows: Vec<Value>,
}

impl BackupTableDump {
    pub fn new(group: impl Into<String>, table: impl Into<String>, rows: Vec<Value>) -> Self {
        Self {
            group: group.into(),
            table: table.into(),
            rows,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupFileEntry {
    pub archive_path: String,
    pub source_path: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupArchiveEntry {
    pub archive_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackupExportRequest {
    pub astrbot_version: String,
    pub exported_at: String,
    pub table_dumps: Vec<BackupTableDump>,
    pub files: Vec<BackupFileEntry>,
    pub directories: BTreeMap<String, BackupDirectoryStat>,
}

impl BackupExportRequest {
    pub fn new(astrbot_version: impl Into<String>, exported_at: impl Into<String>) -> Self {
        Self {
            astrbot_version: astrbot_version.into(),
            exported_at: exported_at.into(),
            table_dumps: Vec::new(),
            files: Vec::new(),
            directories: BTreeMap::new(),
        }
    }

    pub fn with_table_dump(mut self, dump: BackupTableDump) -> Self {
        self.table_dumps.push(dump);
        self
    }

    pub fn with_file(mut self, file: BackupFileEntry) -> Self {
        self.files.push(file);
        self
    }

    pub fn with_directory(mut self, name: impl Into<String>, stat: BackupDirectoryStat) -> Self {
        self.directories.insert(name.into(), stat);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackupExportPackage {
    pub manifest: BackupManifest,
    pub tables: Vec<BackupTableDump>,
    pub files: Vec<BackupFileEntry>,
    pub generated_entries: Vec<BackupArchiveEntry>,
}

impl BackupExportPackage {
    pub fn from_request(request: BackupExportRequest) -> Self {
        let mut manifest = BackupManifest::new(&request.astrbot_version, &request.exported_at);
        let mut table_groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut generated_entries = Vec::new();

        for dump in &request.table_dumps {
            table_groups
                .entry(dump.group.clone())
                .or_default()
                .push(dump.table.clone());
            manifest.add_table_count(&dump.group, &dump.table, dump.rows.len());

            let archive_path = format!("databases/{}/{}.json", dump.group, dump.table);
            let bytes = serde_json::to_vec(&dump.rows).unwrap_or_default();
            manifest.add_checksum(archive_path.clone(), format!("len:{}", bytes.len()));
            generated_entries.push(BackupArchiveEntry {
                archive_path,
                bytes,
            });
        }

        for (group, tables) in table_groups {
            manifest = manifest.with_table_group(group, tables);
        }
        for (name, stat) in request.directories.clone() {
            manifest.add_directory(name, stat);
        }

        Self {
            manifest,
            tables: request.table_dumps,
            files: request.files,
            generated_entries,
        }
    }
}

#[async_trait]
pub trait BackupExportPort: Send + Sync {
    async fn export_backup(&self, request: BackupExportRequest) -> Result<BackupExportPackage>;
}

#[async_trait]
impl BackupExportPort for BackupExportPackage {
    async fn export_backup(&self, _request: BackupExportRequest) -> Result<BackupExportPackage> {
        Ok(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{BackupExportPackage, BackupExportRequest, BackupTableDump};

    #[test]
    fn export_package_builds_manifest_without_route_handlers() {
        let package = BackupExportPackage::from_request(
            BackupExportRequest::new("4.9.0", "2026-05-16T00:00:00Z").with_table_dump(
                BackupTableDump::new(
                    "main_db",
                    "conversations",
                    vec![json!({"conversation_id": "c1"})],
                ),
            ),
        );

        assert_eq!(
            package
                .manifest
                .table_statistics
                .get("main_db")
                .and_then(|tables| tables.get("conversations")),
            Some(&1)
        );
        assert_eq!(
            package.generated_entries[0].archive_path,
            "databases/main_db/conversations.json"
        );
    }
}
