use std::collections::BTreeMap;
use std::path::PathBuf;

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
pub struct BackupVersionCompatibility {
    pub status: BackupVersionStatus,
    pub backup_version: String,
    pub current_version: String,
    pub message: Option<String>,
}

impl BackupVersionCompatibility {
    pub fn compare(backup_version: impl Into<String>, current_version: impl Into<String>) -> Self {
        let backup_version = backup_version.into();
        let current_version = current_version.into();
        let status = compare_major_minor(&backup_version, &current_version);
        let message = match status {
            BackupVersionStatus::Match => None,
            BackupVersionStatus::MinorDiff => Some(format!(
                "backup version {backup_version} differs from current version {current_version}"
            )),
            BackupVersionStatus::MajorDiff => Some(format!(
                "backup version {backup_version} is incompatible with current version {current_version}"
            )),
            BackupVersionStatus::Missing => {
                Some("backup or current version is missing".to_string())
            }
        };

        Self {
            status,
            backup_version,
            current_version,
            message,
        }
    }

    pub fn can_import(&self) -> bool {
        !matches!(
            self.status,
            BackupVersionStatus::MajorDiff | BackupVersionStatus::Missing
        )
    }
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
    #[serde(default, skip_serializing, skip_deserializing)]
    pub source_path: Option<PathBuf>,
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
            source_path: None,
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

    pub fn with_source_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.source_path = Some(path.into());
        self
    }
}

pub(crate) fn compare_major_minor(
    backup_version: &str,
    current_version: &str,
) -> BackupVersionStatus {
    if backup_version.trim().is_empty() || current_version.trim().is_empty() {
        return BackupVersionStatus::Missing;
    }

    let backup_major = major_minor(backup_version);
    let current_major = major_minor(current_version);
    if backup_major != current_major {
        return BackupVersionStatus::MajorDiff;
    }
    if backup_version == current_version {
        BackupVersionStatus::Match
    } else {
        BackupVersionStatus::MinorDiff
    }
}

fn major_minor(version: &str) -> String {
    let normalized = version
        .trim()
        .trim_start_matches('v')
        .split(['-', '+'])
        .next()
        .unwrap_or_default();
    let mut parts = normalized.split('.').filter(|part| !part.is_empty());
    let major = parts.next().unwrap_or("0");
    let minor = parts.next().unwrap_or("0");
    format!("{major}.{minor}")
}

#[cfg(test)]
mod tests {
    use super::{BackupVersionCompatibility, BackupVersionStatus};

    #[test]
    fn version_compatibility_allows_patch_diff_and_rejects_major_minor_line() {
        let same_line = BackupVersionCompatibility::compare("4.9.1", "4.9.2");
        assert!(same_line.can_import());
        assert_eq!(same_line.status, BackupVersionStatus::MinorDiff);

        let incompatible = BackupVersionCompatibility::compare("4.9.1", "4.10.0");
        assert!(!incompatible.can_import());
        assert_eq!(incompatible.status, BackupVersionStatus::MajorDiff);
    }
}
