mod executor;
mod update;

use std::fs;
use std::path::PathBuf;

use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use executor::{
    DependencyInstallOutcomeSummary, FixturePluginMarketPackageFetcher,
    NoopPluginMarketRuntimeReloader, PluginMarketDownloadedPackage, PluginMarketExecutionContext,
    PluginMarketExecutionOptions, PluginMarketExecutionResult, PluginMarketExecutionStatus,
    PluginMarketExecutor, PluginMarketFailureRecord, PluginMarketInstalledMetadata,
    PluginMarketPackageFetcher, PluginMarketRuntimeReloader, PluginMarketStepRecord,
    ReqwestPluginMarketPackageFetcher, derive_repository_archive_url,
};
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

    pub fn from_json_str(value: &str) -> Result<Self> {
        let value = serde_json::from_str::<Value>(value).map_err(|err| {
            AstrbotError::Pipeline(format!("failed to parse plugin market cache JSON: {err}"))
        })?;
        Self::from_json_value(value)
    }

    pub fn from_json_value(value: Value) -> Result<Self> {
        let timestamp = value
            .get("timestamp")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let md5 = value
            .get("md5")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string);
        let data = value.get("data").cloned().unwrap_or(value);
        let plugins = plugin_entries_from_json(data)?;

        Ok(Self {
            timestamp,
            md5,
            plugins,
        })
    }

    pub fn load_from_file(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let content = fs::read_to_string(&path).map_err(|err| {
            AstrbotError::Pipeline(format!(
                "failed to read plugin market cache {}: {err}",
                path.display()
            ))
        })?;
        Self::from_json_str(&content)
    }

    pub fn save_to_file(&self, path: impl Into<PathBuf>) -> Result<()> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                AstrbotError::Pipeline(format!(
                    "failed to create plugin market cache directory {}: {err}",
                    parent.display()
                ))
            })?;
        }
        let content = serde_json::to_string_pretty(self).map_err(|err| {
            AstrbotError::Pipeline(format!("failed to serialize plugin market cache: {err}"))
        })?;
        fs::write(&path, content).map_err(|err| {
            AstrbotError::Pipeline(format!(
                "failed to write plugin market cache {}: {err}",
                path.display()
            ))
        })
    }

    pub fn entry(&self, plugin_id: &str) -> Option<&PluginMarketEntry> {
        let normalized = normalize_plugin_id(plugin_id);
        self.plugins
            .iter()
            .find(|entry| entry.plugin_id == normalized || entry.name == plugin_id)
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

fn plugin_entries_from_json(value: Value) -> Result<Vec<PluginMarketEntry>> {
    match value {
        Value::Array(values) => values
            .into_iter()
            .enumerate()
            .map(|(index, value)| plugin_entry_from_json(index.to_string(), value))
            .collect(),
        Value::Object(values) => values
            .into_iter()
            .map(|(plugin_id, value)| plugin_entry_from_json(plugin_id, value))
            .collect(),
        _ => Err(AstrbotError::Pipeline(
            "plugin market cache data must be an object or array".to_string(),
        )),
    }
}

fn plugin_entry_from_json(plugin_id: String, value: Value) -> Result<PluginMarketEntry> {
    if let Ok(entry) = serde_json::from_value::<PluginMarketEntry>(value.clone()) {
        return Ok(entry);
    }

    let object = value.as_object().ok_or_else(|| {
        AstrbotError::Pipeline(format!("plugin market entry {plugin_id} must be an object"))
    })?;
    let plugin_id = string_field(object, &["plugin_id", "id", "name"])
        .unwrap_or(&plugin_id)
        .to_string();
    let name = string_field(object, &["name", "title"])
        .unwrap_or(plugin_id.as_str())
        .to_string();
    let version = string_field(object, &["version"])
        .unwrap_or("unknown")
        .to_string();
    let mut entry = PluginMarketEntry::new(plugin_id, name, version);

    if let Some(repo) = string_field(object, &["repo", "repo_url", "repository", "url"]) {
        entry = entry.with_repo_url(repo);
    }
    if let Some(package) = object
        .get("package")
        .and_then(|value| serde_json::from_value::<PluginPackageDescriptor>(value.clone()).ok())
        .or_else(|| package_from_json_object(object))
    {
        entry = entry.with_package(package);
    }
    entry = entry.with_compatibility(compatibility_from_json_object(object));
    if let Some(readme) = string_field(object, &["readme", "README", "readme_content"]) {
        entry = entry.with_readme(PluginDocument::markdown(readme));
    }
    if let Some(changelog) = string_field(object, &["changelog", "CHANGELOG"]) {
        entry = entry.with_changelog(PluginDocument::markdown(changelog));
    }

    Ok(entry)
}

fn package_from_json_object(
    object: &serde_json::Map<String, Value>,
) -> Option<PluginPackageDescriptor> {
    if let Some(url) = string_field(object, &["archive_url", "download_url", "zip_url"]) {
        return Some(
            PluginPackageDescriptor::new(PluginInstallSource::archive(url))
                .with_optional_checksum(string_field(object, &["checksum_md5", "md5"]))
                .with_optional_cache_key(string_field(object, &["cache_key"])),
        );
    }
    string_field(object, &["uploaded_archive", "filename"]).map(|filename| {
        PluginPackageDescriptor::new(PluginInstallSource::uploaded_archive(filename))
            .with_optional_checksum(string_field(object, &["checksum_md5", "md5"]))
            .with_optional_cache_key(string_field(object, &["cache_key"]))
    })
}

fn compatibility_from_json_object(object: &serde_json::Map<String, Value>) -> PluginCompatibility {
    if let Some(value) = object.get("compatibility")
        && let Ok(compatibility) = serde_json::from_value::<PluginCompatibility>(value.clone())
    {
        return compatibility;
    }

    let astrbot_version = string_field(object, &["astrbot_version", "required_astrbot_version"])
        .map(ToString::to_string);
    let compatible = bool_field(object, &["compatible", "is_compatible"]).unwrap_or(true);
    let message =
        string_field(object, &["compatibility_message", "message"]).map(ToString::to_string);
    PluginCompatibility {
        astrbot_version,
        compatible,
        message,
    }
}

fn string_field<'a>(object: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .filter_map(|key| object.get(*key))
        .find_map(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn bool_field(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .filter_map(|key| object.get(*key))
        .find_map(Value::as_bool)
}

trait OptionalPackageDescriptorFields {
    fn with_optional_checksum(self, checksum: Option<&str>) -> Self;
    fn with_optional_cache_key(self, cache_key: Option<&str>) -> Self;
}

impl OptionalPackageDescriptorFields for PluginPackageDescriptor {
    fn with_optional_checksum(self, checksum: Option<&str>) -> Self {
        if let Some(checksum) = checksum {
            self.with_checksum_md5(checksum)
        } else {
            self
        }
    }

    fn with_optional_cache_key(self, cache_key: Option<&str>) -> Self {
        if let Some(cache_key) = cache_key {
            self.with_cache_key(cache_key)
        } else {
            self
        }
    }
}
