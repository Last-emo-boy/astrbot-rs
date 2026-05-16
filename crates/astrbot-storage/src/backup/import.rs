use std::collections::BTreeMap;

use astrbot_core::Result;
use async_trait::async_trait;

use super::manifest::{BackupManifest, BackupVersionStatus};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackupImportMode {
    Replace,
    Merge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
        let version_status = compare_major_minor(&manifest.astrbot_version, &current_version);
        let can_import = !matches!(
            version_status,
            BackupVersionStatus::MajorDiff | BackupVersionStatus::Missing
        );
        let mut backup_summary = BTreeMap::new();
        backup_summary.insert("table_groups".to_string(), manifest.tables.len());
        backup_summary.insert("directories".to_string(), manifest.directories.len());
        backup_summary.insert("checksums".to_string(), manifest.checksums.len());

        Self {
            valid: true,
            can_import,
            version_status,
            backup_version: manifest.astrbot_version.clone(),
            current_version,
            backup_time: manifest.exported_at.clone(),
            warnings: Vec::new(),
            error: None,
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

fn compare_major_minor(backup_version: &str, current_version: &str) -> BackupVersionStatus {
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
    use super::{BackupImportPrecheck, BackupVersionStatus};
    use crate::backup::BackupManifest;

    #[test]
    fn import_precheck_allows_minor_patch_difference_only() {
        let manifest = BackupManifest::new("4.9.1", "2026-05-16T00:00:00Z");
        let precheck = BackupImportPrecheck::from_manifest(&manifest, "4.9.2");

        assert!(precheck.can_import);
        assert_eq!(precheck.version_status, BackupVersionStatus::MinorDiff);

        let rejected = BackupImportPrecheck::from_manifest(&manifest, "4.10.0");
        assert!(!rejected.can_import);
        assert_eq!(rejected.version_status, BackupVersionStatus::MajorDiff);
    }
}
