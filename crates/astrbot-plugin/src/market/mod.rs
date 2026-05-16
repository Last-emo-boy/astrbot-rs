mod update;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use update::{
    PluginInstallPlan, PluginMarketAction, PluginMarketOperationPlan, PluginUninstallPlan,
    PluginUpdatePlan,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRegistrySource {
    pub urls: Vec<String>,
    pub cache_file: PathBuf,
    pub md5_url: Option<String>,
}

impl PluginRegistrySource {
    pub fn new(
        urls: impl IntoIterator<Item = impl Into<String>>,
        cache_file: impl Into<PathBuf>,
    ) -> Self {
        Self {
            urls: urls.into_iter().map(Into::into).collect(),
            cache_file: cache_file.into(),
            md5_url: None,
        }
    }

    pub fn with_md5_url(mut self, md5_url: impl Into<String>) -> Self {
        let md5_url = md5_url.into();
        self.md5_url = (!md5_url.trim().is_empty()).then_some(md5_url);
        self
    }

    pub fn default_collection(cache_file: impl Into<PathBuf>) -> Self {
        Self::new(
            [
                "https://api.soulter.top/astrbot/plugins",
                "https://github.com/AstrBotDevs/AstrBot_Plugins_Collection/raw/refs/heads/main/plugin_cache_original.json",
            ],
            cache_file,
        )
        .with_md5_url("https://api.soulter.top/astrbot/plugins-md5")
    }

    pub fn custom(url: impl Into<String>, cache_file: impl Into<PathBuf>) -> Self {
        let url = url.into();
        let md5_url = if let Some(prefix) = url.strip_suffix(".json") {
            format!("{prefix}-md5.json")
        } else {
            format!("{url}-md5.json")
        };
        Self::new([url], cache_file).with_md5_url(md5_url)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMarketCache {
    pub timestamp: String,
    pub md5: Option<String>,
    pub plugins: Vec<PluginMarketEntry>,
}

impl PluginMarketCache {
    pub fn new(timestamp: impl Into<String>, plugins: Vec<PluginMarketEntry>) -> Self {
        Self {
            timestamp: timestamp.into(),
            md5: None,
            plugins,
        }
    }

    pub fn with_md5(mut self, md5: impl Into<String>) -> Self {
        let md5 = md5.into();
        self.md5 = (!md5.trim().is_empty()).then_some(md5);
        self
    }

    pub fn is_valid_for_remote_md5(&self, remote_md5: Option<&str>) -> bool {
        match remote_md5 {
            Some(remote) => self.md5.as_deref() == Some(remote),
            None => self.md5.is_some(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMarketEntry {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub repo_url: Option<String>,
    pub package: Option<PluginPackageDescriptor>,
    pub compatibility: PluginCompatibility,
    pub readme: Option<PluginDocument>,
    pub changelog: Option<PluginDocument>,
}

impl PluginMarketEntry {
    pub fn new(
        plugin_id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            plugin_id: normalize_plugin_id(&plugin_id.into()),
            name: name.into(),
            version: version.into(),
            repo_url: None,
            package: None,
            compatibility: PluginCompatibility::unknown(),
            readme: None,
            changelog: None,
        }
    }

    pub fn with_repo_url(mut self, repo_url: impl Into<String>) -> Self {
        let repo_url = repo_url.into();
        self.repo_url = (!repo_url.trim().is_empty()).then_some(repo_url);
        self
    }

    pub fn with_package(mut self, package: PluginPackageDescriptor) -> Self {
        self.package = Some(package);
        self
    }

    pub fn with_compatibility(mut self, compatibility: PluginCompatibility) -> Self {
        self.compatibility = compatibility;
        self
    }

    pub fn with_readme(mut self, document: PluginDocument) -> Self {
        self.readme = Some(document);
        self
    }

    pub fn with_changelog(mut self, document: PluginDocument) -> Self {
        self.changelog = Some(document);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPackageDescriptor {
    pub source: PluginInstallSource,
    pub checksum_md5: Option<String>,
    pub cache_key: Option<String>,
}

impl PluginPackageDescriptor {
    pub fn new(source: PluginInstallSource) -> Self {
        Self {
            source,
            checksum_md5: None,
            cache_key: None,
        }
    }

    pub fn with_checksum_md5(mut self, checksum_md5: impl Into<String>) -> Self {
        let checksum_md5 = checksum_md5.into();
        self.checksum_md5 = (!checksum_md5.trim().is_empty()).then_some(checksum_md5);
        self
    }

    pub fn with_cache_key(mut self, cache_key: impl Into<String>) -> Self {
        let cache_key = cache_key.into();
        self.cache_key = (!cache_key.trim().is_empty()).then_some(cache_key);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginInstallSource {
    Repository { url: String },
    Archive { url: String },
    UploadedArchive { filename: String },
}

impl PluginInstallSource {
    pub fn repository(url: impl Into<String>) -> Self {
        Self::Repository { url: url.into() }
    }

    pub fn archive(url: impl Into<String>) -> Self {
        Self::Archive { url: url.into() }
    }

    pub fn uploaded_archive(filename: impl Into<String>) -> Self {
        Self::UploadedArchive {
            filename: filename.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCompatibility {
    pub astrbot_version: Option<String>,
    pub compatible: bool,
    pub message: Option<String>,
}

impl PluginCompatibility {
    pub fn unknown() -> Self {
        Self {
            astrbot_version: None,
            compatible: true,
            message: None,
        }
    }

    pub fn compatible(astrbot_version: impl Into<String>) -> Self {
        Self {
            astrbot_version: Some(astrbot_version.into()),
            compatible: true,
            message: None,
        }
    }

    pub fn incompatible(astrbot_version: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            astrbot_version: Some(astrbot_version.into()),
            compatible: false,
            message: Some(message.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginDocument {
    pub format: PluginDocumentFormat,
    pub content: Option<String>,
    pub source_path: Option<String>,
}

impl PluginDocument {
    pub fn markdown(content: impl Into<String>) -> Self {
        Self {
            format: PluginDocumentFormat::Markdown,
            content: Some(content.into()),
            source_path: None,
        }
    }

    pub fn missing() -> Self {
        Self {
            format: PluginDocumentFormat::Markdown,
            content: None,
            source_path: None,
        }
    }

    pub fn with_source_path(mut self, source_path: impl Into<String>) -> Self {
        let source_path = source_path.into();
        self.source_path = (!source_path.trim().is_empty()).then_some(source_path);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginDocumentFormat {
    Markdown,
    PlainText,
}

fn normalize_plugin_id(value: &str) -> String {
    let mut normalized = String::new();
    let mut last_was_separator = false;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            normalized.push('_');
            last_was_separator = true;
        }
    }
    let normalized = normalized.trim_matches('_').to_string();
    if normalized.is_empty() {
        "plugin".to_string()
    } else {
        normalized
    }
}
