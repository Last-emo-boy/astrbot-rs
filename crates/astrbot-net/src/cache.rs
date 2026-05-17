use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadCacheKey {
    pub namespace: String,
    pub version: Option<String>,
    pub file_name: String,
}

impl DownloadCacheKey {
    pub fn new(namespace: impl Into<String>, file_name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            version: None,
            file_name: file_name.into(),
        }
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        let version = version.into();
        if !version.trim().is_empty() {
            self.version = Some(version);
        }
        self
    }

    pub fn relative_path(&self) -> PathBuf {
        let mut path = PathBuf::from(safe_segment(&self.namespace));
        if let Some(version) = &self.version {
            path = path.join(safe_segment(version));
        }
        path.join(safe_file_name(&self.file_name))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadCachePolicy {
    pub cache_dir: PathBuf,
    pub use_cache: bool,
    pub refresh: bool,
}

impl DownloadCachePolicy {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            use_cache: true,
            refresh: false,
        }
    }

    pub fn cache_path(&self, key: &DownloadCacheKey) -> PathBuf {
        self.cache_dir.join(key.relative_path())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadCacheRecord {
    pub key: DownloadCacheKey,
    pub path: PathBuf,
    pub bytes: u64,
    pub created_at: Option<SystemTime>,
}

fn safe_segment(value: &str) -> String {
    let safe = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if safe.is_empty() {
        "download".to_string()
    } else {
        safe
    }
}

fn safe_file_name(value: &str) -> String {
    value
        .replace('\\', "/")
        .split('/')
        .rev()
        .find(|segment| !segment.trim().is_empty() && *segment != "." && *segment != "..")
        .map(safe_segment)
        .filter(|segment| !segment.is_empty())
        .unwrap_or_else(|| "download".to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{DownloadCacheKey, DownloadCachePolicy};

    #[test]
    fn cache_key_sanitizes_namespace_version_and_file_name() {
        let key = DownloadCacheKey::new("dashboard release", "../dist.zip").with_version("v 1");
        let policy = DownloadCachePolicy::new("data/cache");

        assert_eq!(
            policy.cache_path(&key),
            PathBuf::from("data/cache/dashboard_release/v_1/dist.zip")
        );
    }
}
