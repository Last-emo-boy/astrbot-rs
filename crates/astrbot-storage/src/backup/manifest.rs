use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const BACKUP_MANIFEST_VERSION: &str = "1.1";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupSchemaVersion {
    pub main_db: String,
    pub kb_db: Option<String>,
}

impl BackupSchemaVersion {
    pub fn main_v4() -> Self {
        Self {
            main_db: "v4".to_string(),
            kb_db: Some("v1".to_string()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupDirectoryStat {
    pub files: usize,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupVersionStatus {
    Match,
    MinorDiff,
    MajorDiff,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub version: String,
    pub astrbot_version: String,
    pub exported_at: String,
    pub origin: String,
    pub schema_version: BackupSchemaVersion,
    pub tables: BTreeMap<String, Vec<String>>,
    pub directories: Vec<String>,
    pub checksums: BTreeMap<String, String>,
    pub table_statistics: BTreeMap<String, BTreeMap<String, usize>>,
    pub directory_statistics: BTreeMap<String, BackupDirectoryStat>,
}

impl BackupManifest {
    pub fn new(astrbot_version: impl Into<String>, exported_at: impl Into<String>) -> Self {
        Self {
            version: BACKUP_MANIFEST_VERSION.to_string(),
            astrbot_version: astrbot_version.into(),
            exported_at: exported_at.into(),
            origin: "exported".to_string(),
            schema_version: BackupSchemaVersion::main_v4(),
            tables: BTreeMap::new(),
            directories: Vec::new(),
            checksums: BTreeMap::new(),
            table_statistics: BTreeMap::new(),
            directory_statistics: BTreeMap::new(),
        }
    }

    pub fn with_table_group<I, S>(mut self, group: impl Into<String>, tables: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tables
            .insert(group.into(), tables.into_iter().map(Into::into).collect());
        self
    }

    pub fn add_table_count(
        &mut self,
        group: impl Into<String>,
        table: impl Into<String>,
        count: usize,
    ) {
        self.table_statistics
            .entry(group.into())
            .or_default()
            .insert(table.into(), count);
    }

    pub fn add_checksum(&mut self, path: impl Into<String>, checksum: impl Into<String>) {
        self.checksums.insert(path.into(), checksum.into());
    }

    pub fn add_directory(&mut self, name: impl Into<String>, stat: BackupDirectoryStat) {
        let name = name.into();
        if !self.directories.contains(&name) {
            self.directories.push(name.clone());
        }
        self.directory_statistics.insert(name, stat);
    }
}
