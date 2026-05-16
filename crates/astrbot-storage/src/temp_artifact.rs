use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use astrbot_core::{AstrbotError, Result};

const DEFAULT_TEMP_MAX_BYTES: u64 = 1024 * 1024 * 1024;
const DEFAULT_CLEANUP_RATIO_PERCENT: u8 = 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TempArtifactRoot {
    root_dir: PathBuf,
}

impl TempArtifactRoot {
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: normalize_path(root_dir.into()),
        }
    }

    pub fn from_astrbot_root(root_dir: impl Into<PathBuf>) -> Self {
        Self::new(root_dir.into().join("data").join("temp"))
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn bucket(&self, bucket: impl AsRef<str>) -> PathBuf {
        self.root_dir.join(safe_artifact_segment(bucket.as_ref()))
    }

    pub fn allocate(
        &self,
        bucket: impl AsRef<str>,
        file_name: impl AsRef<str>,
    ) -> TempArtifactDescriptor {
        let bucket = safe_artifact_segment(bucket.as_ref());
        let file_name = safe_artifact_segment(file_name.as_ref());
        TempArtifactDescriptor {
            bucket: bucket.clone(),
            relative_path: PathBuf::from(&bucket).join(&file_name),
            path: self.root_dir.join(bucket).join(file_name),
        }
    }
}

impl Default for TempArtifactRoot {
    fn default() -> Self {
        Self::from_astrbot_root(default_astrbot_root())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TempArtifactDescriptor {
    pub bucket: String,
    pub relative_path: PathBuf,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TempArtifactCleanupPolicy {
    pub max_bytes: u64,
    pub cleanup_ratio_percent: u8,
}

impl TempArtifactCleanupPolicy {
    pub fn new(max_bytes: u64) -> Self {
        Self {
            max_bytes,
            cleanup_ratio_percent: DEFAULT_CLEANUP_RATIO_PERCENT,
        }
    }

    pub fn with_cleanup_ratio_percent(mut self, cleanup_ratio_percent: u8) -> Self {
        self.cleanup_ratio_percent = cleanup_ratio_percent.clamp(1, 100);
        self
    }

    pub fn plan(&self, files: Vec<TempArtifactFileInfo>) -> TempArtifactCleanupPlan {
        let total_bytes = files.iter().map(|file| file.size).sum::<u64>();
        if total_bytes <= self.max_bytes {
            return TempArtifactCleanupPlan {
                total_bytes,
                limit_bytes: self.max_bytes,
                target_release_bytes: 0,
                files_to_remove: Vec::new(),
            };
        }

        let target_release_bytes =
            ((total_bytes * u64::from(self.cleanup_ratio_percent)) / 100).max(1);
        let mut files = files;
        files.sort_by_key(|file| file.modified_at);

        let mut released = 0;
        let mut files_to_remove = Vec::new();
        for file in files {
            released += file.size;
            files_to_remove.push(file.path);
            if released >= target_release_bytes {
                break;
            }
        }

        TempArtifactCleanupPlan {
            total_bytes,
            limit_bytes: self.max_bytes,
            target_release_bytes,
            files_to_remove,
        }
    }
}

impl Default for TempArtifactCleanupPolicy {
    fn default() -> Self {
        Self::new(DEFAULT_TEMP_MAX_BYTES)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TempArtifactFileInfo {
    pub path: PathBuf,
    pub size: u64,
    pub modified_at: SystemTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TempArtifactCleanupPlan {
    pub total_bytes: u64,
    pub limit_bytes: u64,
    pub target_release_bytes: u64,
    pub files_to_remove: Vec<PathBuf>,
}

impl TempArtifactCleanupPlan {
    pub fn is_empty(&self) -> bool {
        self.files_to_remove.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct TempArtifactCleaner {
    root: TempArtifactRoot,
    policy: TempArtifactCleanupPolicy,
}

impl TempArtifactCleaner {
    pub fn new(root: TempArtifactRoot, policy: TempArtifactCleanupPolicy) -> Self {
        Self { root, policy }
    }

    pub fn cleanup_plan(&self) -> Result<TempArtifactCleanupPlan> {
        Ok(self.policy.plan(scan_temp_files(self.root.root_dir())?))
    }

    pub fn cleanup_once(&self) -> Result<TempArtifactCleanupPlan> {
        let plan = self.cleanup_plan()?;
        for path in &plan.files_to_remove {
            fs::remove_file(path).map_err(|err| {
                AstrbotError::Pipeline(format!(
                    "remove temp artifact {}: {err}",
                    path.to_string_lossy()
                ))
            })?;
        }
        cleanup_empty_dirs(self.root.root_dir())?;
        Ok(plan)
    }
}

pub fn safe_artifact_segment(value: &str) -> String {
    let safe = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    if safe.is_empty() {
        "artifact".to_string()
    } else {
        safe
    }
}

fn scan_temp_files(root: &Path) -> Result<Vec<TempArtifactFileInfo>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    scan_temp_files_inner(root, &mut files)?;
    Ok(files)
}

fn scan_temp_files_inner(root: &Path, files: &mut Vec<TempArtifactFileInfo>) -> Result<()> {
    for entry in fs::read_dir(root).map_err(|err| {
        AstrbotError::Pipeline(format!(
            "read temp artifact directory {}: {err}",
            root.display()
        ))
    })? {
        let entry = entry.map_err(|err| {
            AstrbotError::Pipeline(format!("read temp artifact directory entry: {err}"))
        })?;
        let path = entry.path();
        let metadata = entry.metadata().map_err(|err| {
            AstrbotError::Pipeline(format!("stat temp artifact {}: {err}", path.display()))
        })?;
        if metadata.is_dir() {
            scan_temp_files_inner(&path, files)?;
            continue;
        }
        if metadata.is_file() {
            files.push(TempArtifactFileInfo {
                path,
                size: metadata.len(),
                modified_at: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }
    Ok(())
}

fn cleanup_empty_dirs(root: &Path) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    cleanup_empty_dirs_inner(root)?;
    Ok(())
}

fn cleanup_empty_dirs_inner(root: &Path) -> Result<bool> {
    let mut is_empty = true;
    for entry in fs::read_dir(root).map_err(|err| {
        AstrbotError::Pipeline(format!(
            "read temp artifact directory {}: {err}",
            root.display()
        ))
    })? {
        let entry = entry.map_err(|err| {
            AstrbotError::Pipeline(format!("read temp artifact directory entry: {err}"))
        })?;
        let path = entry.path();
        if path.is_dir() {
            if cleanup_empty_dirs_inner(&path)? {
                fs::remove_dir(&path).map_err(|err| {
                    AstrbotError::Pipeline(format!(
                        "remove empty temp artifact directory {}: {err}",
                        path.display()
                    ))
                })?;
            } else {
                is_empty = false;
            }
        } else {
            is_empty = false;
        }
    }

    Ok(is_empty)
}

fn default_astrbot_root() -> PathBuf {
    std::env::var_os("ASTRBOT_ROOT")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use super::{
        TempArtifactCleanupPolicy, TempArtifactFileInfo, TempArtifactRoot, safe_artifact_segment,
    };

    #[test]
    fn temp_root_matches_astrbot_data_temp_shape() {
        let root = TempArtifactRoot::from_astrbot_root("workspace");

        assert_eq!(root.root_dir(), &PathBuf::from("workspace/data/temp"));
        assert_eq!(
            root.bucket("generated media"),
            PathBuf::from("workspace/data/temp/generated_media")
        );
    }

    #[test]
    fn allocates_sanitized_relative_artifact_path() {
        let root = TempArtifactRoot::from_astrbot_root("workspace");

        let artifact = root.allocate("quoted image", "reply 1.png");

        assert_eq!(artifact.bucket, "quoted_image");
        assert_eq!(
            artifact.relative_path,
            PathBuf::from("quoted_image/reply_1_png")
        );
        assert_eq!(
            artifact.path,
            PathBuf::from("workspace/data/temp/quoted_image/reply_1_png")
        );
    }

    #[test]
    fn cleanup_policy_selects_oldest_files_until_target_release() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let policy = TempArtifactCleanupPolicy::new(100).with_cleanup_ratio_percent(30);

        let plan = policy.plan(vec![
            TempArtifactFileInfo {
                path: PathBuf::from("new"),
                size: 80,
                modified_at: now + Duration::from_secs(2),
            },
            TempArtifactFileInfo {
                path: PathBuf::from("old"),
                size: 40,
                modified_at: now,
            },
            TempArtifactFileInfo {
                path: PathBuf::from("middle"),
                size: 30,
                modified_at: now + Duration::from_secs(1),
            },
        ]);

        assert_eq!(plan.total_bytes, 150);
        assert_eq!(plan.target_release_bytes, 45);
        assert_eq!(
            plan.files_to_remove,
            vec![PathBuf::from("old"), PathBuf::from("middle")]
        );
    }

    #[test]
    fn safe_segment_falls_back_for_blank_or_symbols() {
        assert_eq!(safe_artifact_segment("  "), "artifact");
        assert_eq!(safe_artifact_segment("a/b:c"), "a_b_c");
    }
}
