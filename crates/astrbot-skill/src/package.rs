use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{SkillCatalog, SkillPackageOperation, is_valid_skill_name};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkillPackageError {
    EmptyArchive,
    MultipleTopLevelFolders,
    InvalidSkillName {
        name: String,
    },
    AbsolutePath {
        path: String,
    },
    InvalidRelativePath {
        path: String,
    },
    UnexpectedTopLevelEntry {
        path: String,
    },
    MissingSkillManifest {
        skill_name: String,
    },
    SandboxOnlyMutation {
        name: String,
        operation: SkillPackageOperation,
    },
    SkillAlreadyExists {
        name: String,
    },
    SkillNotFound {
        name: String,
    },
}

impl SkillPackageError {
    pub fn invalid_skill_name(name: impl Into<String>) -> Self {
        Self::InvalidSkillName { name: name.into() }
    }

    pub fn sandbox_only_mutation(
        name: impl Into<String>,
        operation: SkillPackageOperation,
    ) -> Self {
        Self::SandboxOnlyMutation {
            name: name.into(),
            operation,
        }
    }
}

impl fmt::Display for SkillPackageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyArchive => write!(formatter, "zip archive is empty"),
            Self::MultipleTopLevelFolders => {
                write!(
                    formatter,
                    "zip archive must contain a single top-level folder"
                )
            }
            Self::InvalidSkillName { name } => write!(formatter, "invalid skill name: {name}"),
            Self::AbsolutePath { path } => {
                write!(formatter, "zip archive contains an absolute path: {path}")
            }
            Self::InvalidRelativePath { path } => {
                write!(
                    formatter,
                    "zip archive contains an invalid relative path: {path}"
                )
            }
            Self::UnexpectedTopLevelEntry { path } => {
                write!(
                    formatter,
                    "zip archive contains an unexpected top-level entry: {path}"
                )
            }
            Self::MissingSkillManifest { skill_name } => {
                write!(
                    formatter,
                    "SKILL.md not found in skill folder: {skill_name}"
                )
            }
            Self::SandboxOnlyMutation { name, operation } => {
                write!(
                    formatter,
                    "sandbox-only skill {name} cannot be changed by local {operation:?}"
                )
            }
            Self::SkillAlreadyExists { name } => write!(formatter, "skill already exists: {name}"),
            Self::SkillNotFound { name } => write!(formatter, "skill not found: {name}"),
        }
    }
}

impl Error for SkillPackageError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillZipValidation {
    pub skill_name: String,
    pub manifest_path: String,
    pub file_count: usize,
    pub ignored_entry_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPackageInstallPlan {
    pub skill_name: String,
    pub overwrite: bool,
    pub manifest_path: String,
    pub file_count: usize,
    pub requires_unpack: bool,
    pub requires_activation_write: bool,
}

impl SkillPackageInstallPlan {
    pub fn from_zip_entries<I, S>(entries: I, overwrite: bool) -> Result<Self, SkillPackageError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let validation = validate_skill_zip_entries(entries)?;
        Ok(Self {
            skill_name: validation.skill_name,
            overwrite,
            manifest_path: validation.manifest_path,
            file_count: validation.file_count,
            requires_unpack: true,
            requires_activation_write: true,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPackageDeletePlan {
    pub skill_name: String,
    pub remove_local_dir: bool,
    pub remove_activation_config: bool,
    pub remove_sandbox_cache_entry: bool,
}

impl SkillPackageDeletePlan {
    pub fn from_catalog(
        catalog: &SkillCatalog,
        skill_name: impl Into<String>,
    ) -> Result<Self, SkillPackageError> {
        let skill_name = skill_name.into();
        ensure_local_package_mutation(catalog, &skill_name, SkillPackageOperation::Delete)?;
        Ok(Self {
            skill_name,
            remove_local_dir: true,
            remove_activation_config: true,
            remove_sandbox_cache_entry: true,
        })
    }
}

pub fn ensure_local_package_mutation(
    catalog: &SkillCatalog,
    skill_name: &str,
    operation: SkillPackageOperation,
) -> Result<(), SkillPackageError> {
    if !is_valid_skill_name(skill_name) {
        return Err(SkillPackageError::invalid_skill_name(skill_name));
    }
    let Some(skill) = catalog.skill(skill_name) else {
        return Err(SkillPackageError::SkillNotFound {
            name: skill_name.to_string(),
        });
    };
    if skill.is_sandbox_only() {
        return Err(SkillPackageError::sandbox_only_mutation(
            skill_name, operation,
        ));
    }
    Ok(())
}

pub fn validate_skill_zip_entries<I, S>(entries: I) -> Result<SkillZipValidation, SkillPackageError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut normalized = Vec::new();
    let mut ignored_entry_count = 0;
    for entry in entries {
        let entry = normalize_zip_entry(entry.into());
        if entry.is_empty() {
            continue;
        }
        if is_ignored_zip_entry(&entry) {
            ignored_entry_count += 1;
            continue;
        }
        normalized.push(entry);
    }

    let file_names = normalized
        .iter()
        .filter(|entry| !entry.ends_with('/'))
        .cloned()
        .collect::<Vec<_>>();
    if file_names.is_empty() {
        return Err(SkillPackageError::EmptyArchive);
    }

    for entry in &normalized {
        if is_absolute_zip_entry(entry) {
            return Err(SkillPackageError::AbsolutePath {
                path: entry.clone(),
            });
        }
        if zip_entry_parts(entry).contains(&"..") {
            return Err(SkillPackageError::InvalidRelativePath {
                path: entry.clone(),
            });
        }
    }

    let mut top_dirs = file_names
        .iter()
        .filter_map(|entry| {
            zip_entry_parts(entry)
                .first()
                .map(|part| (*part).to_string())
        })
        .collect::<Vec<_>>();
    top_dirs.sort();
    top_dirs.dedup();
    if top_dirs.len() != 1 {
        return Err(SkillPackageError::MultipleTopLevelFolders);
    }

    let skill_name = top_dirs.remove(0);
    if !is_valid_skill_name(&skill_name) {
        return Err(SkillPackageError::invalid_skill_name(skill_name));
    }

    for entry in &normalized {
        let parts = zip_entry_parts(entry);
        if parts.first().is_some_and(|top| *top != skill_name) {
            return Err(SkillPackageError::UnexpectedTopLevelEntry {
                path: entry.clone(),
            });
        }
    }

    let manifest_path = file_names
        .iter()
        .find(|entry| {
            let parts = zip_entry_parts(entry);
            parts.len() == 2 && parts[0] == skill_name && parts[1].eq_ignore_ascii_case("SKILL.md")
        })
        .cloned()
        .ok_or_else(|| SkillPackageError::MissingSkillManifest {
            skill_name: skill_name.clone(),
        })?;

    Ok(SkillZipValidation {
        skill_name,
        manifest_path,
        file_count: file_names.len(),
        ignored_entry_count,
    })
}

fn normalize_zip_entry(entry: String) -> String {
    entry.replace('\\', "/")
}

fn is_ignored_zip_entry(entry: &str) -> bool {
    zip_entry_parts(entry)
        .iter()
        .any(|part| *part == "__MACOSX" || *part == ".DS_Store")
}

fn is_absolute_zip_entry(entry: &str) -> bool {
    entry.starts_with('/') || has_windows_drive_prefix(entry)
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn zip_entry_parts(entry: &str) -> Vec<&str> {
    entry.split('/').filter(|part| !part.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use crate::{
        SkillCatalog, SkillDescriptor, SkillPackageDeletePlan, SkillPackageError,
        SkillPackageInstallPlan, SkillSource, validate_skill_zip_entries,
    };

    #[test]
    fn zip_validation_accepts_single_skill_folder_with_manifest() {
        let validation = validate_skill_zip_entries([
            "__MACOSX/ignored",
            ".DS_Store",
            "writer/.DS_Store",
            "writer/",
            "writer/SKILL.md",
            "writer/assets/template.txt",
        ])
        .expect("valid skill zip entries should pass");

        assert_eq!(validation.skill_name, "writer");
        assert_eq!(validation.manifest_path, "writer/SKILL.md");
        assert_eq!(validation.file_count, 2);
        assert_eq!(validation.ignored_entry_count, 3);
    }

    #[test]
    fn zip_validation_rejects_absolute_and_parent_paths() {
        let absolute = validate_skill_zip_entries(["/writer/SKILL.md"])
            .expect_err("absolute path should be rejected");
        assert!(matches!(absolute, SkillPackageError::AbsolutePath { .. }));

        let relative = validate_skill_zip_entries(["writer/../SKILL.md"])
            .expect_err("parent path should be rejected");
        assert!(matches!(
            relative,
            SkillPackageError::InvalidRelativePath { .. }
        ));

        let drive = validate_skill_zip_entries(["C:/writer/SKILL.md"])
            .expect_err("windows drive path should be rejected");
        assert!(matches!(drive, SkillPackageError::AbsolutePath { .. }));
    }

    #[test]
    fn zip_validation_requires_single_top_folder_and_skill_manifest() {
        let multi = validate_skill_zip_entries(["one/SKILL.md", "two/SKILL.md"])
            .expect_err("multiple top folders should be rejected");
        assert!(matches!(multi, SkillPackageError::MultipleTopLevelFolders));

        let missing = validate_skill_zip_entries(["writer/README.md"])
            .expect_err("missing manifest should be rejected");
        assert!(matches!(
            missing,
            SkillPackageError::MissingSkillManifest { .. }
        ));
    }

    #[test]
    fn install_plan_is_side_effect_free_zip_policy() {
        let plan = SkillPackageInstallPlan::from_zip_entries(["writer/SKILL.md"], true)
            .expect("install plan should validate zip entries");

        assert_eq!(plan.skill_name, "writer");
        assert!(plan.overwrite);
        assert!(plan.requires_unpack);
        assert!(plan.requires_activation_write);
    }

    #[test]
    fn delete_plan_rejects_sandbox_only_skill() {
        let catalog = SkillCatalog::from_skills([SkillDescriptor::new(
            "preset",
            "/workspace/skills/preset/SKILL.md",
        )
        .with_source(SkillSource::Sandbox)]);

        let error = SkillPackageDeletePlan::from_catalog(&catalog, "preset")
            .expect_err("sandbox-only skill should not have local delete plan");

        assert!(matches!(
            error,
            SkillPackageError::SandboxOnlyMutation { .. }
        ));
    }
}
