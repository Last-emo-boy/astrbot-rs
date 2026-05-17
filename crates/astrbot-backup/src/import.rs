use std::collections::BTreeMap;

use astrbot_core::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::manifest::{BackupManifest, BackupVersionCompatibility, BackupVersionStatus};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupImportMode {
    Replace,
    Merge,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupImportPrecheck {
    pub valid: bool,
    pub can_import: bool,
    pub version_status: BackupVersionStatus,
    pub backup_version: String,
    pub current_version: String,
    pub backup_time: String,
    pub warnings: Vec<String>,
    pub error: Option<String>,
    pub backup_summary: BTreeMap<String, usize>,
}

impl BackupImportPrecheck {
    pub fn from_manifest(manifest: &BackupManifest, current_version: impl Into<String>) -> Self {
        let current_version = current_version.into();
        let compatibility =
            BackupVersionCompatibility::compare(&manifest.astrbot_version, &current_version);
        let mut backup_summary = BTreeMap::new();
        backup_summary.insert("table_groups".to_string(), manifest.tables.len());
        backup_summary.insert("directories".to_string(), manifest.directories.len());
        backup_summary.insert("checksums".to_string(), manifest.checksums.len());

        let warnings = compatibility
            .message
            .clone()
            .filter(|_| compatibility.status == BackupVersionStatus::MinorDiff)
            .into_iter()
            .collect();
        let error = compatibility
            .message
            .clone()
            .filter(|_| !compatibility.can_import());

        Self {
            valid: true,
            can_import: compatibility.can_import(),
            version_status: compatibility.status,
            backup_version: manifest.astrbot_version.clone(),
            current_version,
            backup_time: manifest.exported_at.clone(),
            warnings,
            error,
            backup_summary,
        }
    }

    pub fn invalid(error: impl Into<String>, current_version: impl Into<String>) -> Self {
        Self {
            valid: false,
            can_import: false,
            version_status: BackupVersionStatus::Missing,
            backup_version: String::new(),
            current_version: current_version.into(),
            backup_time: String::new(),
            warnings: Vec::new(),
            error: Some(error.into()),
            backup_summary: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupImportResult {
    pub success: bool,
    pub imported_tables: BTreeMap<String, usize>,
    pub imported_files: BTreeMap<String, usize>,
    pub imported_directories: BTreeMap<String, usize>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl BackupImportResult {
    pub fn success() -> Self {
        Self {
            success: true,
            ..Self::default()
        }
    }

    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    pub fn add_error(&mut self, error: impl Into<String>) {
        self.errors.push(error.into());
        self.success = false;
    }
}

#[async_trait]
pub trait BackupImportPort: Send + Sync {
    async fn precheck_backup(&self, manifest: &BackupManifest) -> Result<BackupImportPrecheck>;

    async fn import_backup(
        &self,
        manifest: BackupManifest,
        mode: BackupImportMode,
    ) -> Result<BackupImportResult>;
}

#[cfg(test)]
mod tests {
    use super::{BackupImportPrecheck, BackupVersionStatus};
    use crate::BackupManifest;

    #[test]
    fn import_precheck_allows_minor_patch_difference_only() {
        let manifest = BackupManifest::new("4.9.1", "2026-05-16T00:00:00Z");
        let precheck = BackupImportPrecheck::from_manifest(&manifest, "4.9.2");

        assert!(precheck.can_import);
        assert_eq!(precheck.version_status, BackupVersionStatus::MinorDiff);
        assert_eq!(precheck.warnings.len(), 1);

        let rejected = BackupImportPrecheck::from_manifest(&manifest, "4.10.0");
        assert!(!rejected.can_import);
        assert_eq!(rejected.version_status, BackupVersionStatus::MajorDiff);
        assert!(rejected.error.is_some());
    }
}
