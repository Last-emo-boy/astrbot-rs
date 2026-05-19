use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::types::{McpError, McpResult, McpUri};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRootsCapabilityConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

impl McpRootsCapabilityConfig {
    pub fn enabled_for_default_paths() -> Self {
        Self {
            enabled: true,
            paths: vec![
                McpRootAlias::Data.as_str().to_string(),
                McpRootAlias::Temp.as_str().to_string(),
            ],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRootsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRoot {
    pub uri: McpUri,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl McpRoot {
    pub fn new(uri: McpUri) -> Self {
        Self { uri, name: None }
    }

    pub fn named(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.name = (!name.trim().is_empty()).then_some(name);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpRootAlias {
    Root,
    Data,
    Config,
    Plugins,
    PluginData,
    Temp,
    Skills,
    KnowledgeBase,
    Backups,
}

impl McpRootAlias {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Data => "data",
            Self::Config => "config",
            Self::Plugins => "plugins",
            Self::PluginData => "plugin_data",
            Self::Temp => "temp",
            Self::Skills => "skills",
            Self::KnowledgeBase => "knowledge_base",
            Self::Backups => "backups",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Root,
            Self::Data,
            Self::Config,
            Self::Plugins,
            Self::PluginData,
            Self::Temp,
            Self::Skills,
            Self::KnowledgeBase,
            Self::Backups,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpRootResolver {
    project_root: PathBuf,
    config: McpRootsCapabilityConfig,
    explicit_allowlist: BTreeSet<PathBuf>,
}

impl McpRootResolver {
    pub fn new(project_root: impl Into<PathBuf>, config: McpRootsCapabilityConfig) -> Self {
        Self {
            project_root: project_root.into(),
            config,
            explicit_allowlist: BTreeSet::new(),
        }
    }

    pub fn with_allowed_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.explicit_allowlist.insert(normalize_path(path.into()));
        self
    }

    pub fn resolve(&self) -> McpResult<Vec<McpRoot>> {
        if !self.config.enabled {
            return Ok(Vec::new());
        }
        let entries = if self.config.paths.is_empty() {
            vec![
                McpRootAlias::Data.as_str().to_string(),
                McpRootAlias::Temp.as_str().to_string(),
            ]
        } else {
            self.config.paths.clone()
        };
        let mut seen = BTreeSet::new();
        let mut roots = Vec::new();
        for entry in entries {
            let Some((name, path)) = self.resolve_entry(&entry)? else {
                continue;
            };
            if !seen.insert(path.clone()) {
                continue;
            }
            roots.push(McpRoot::new(path_to_file_uri(&path)?).named(name));
        }
        Ok(roots)
    }

    fn resolve_entry(&self, entry: &str) -> McpResult<Option<(String, PathBuf)>> {
        let entry = entry.trim();
        if entry.is_empty() {
            return Ok(None);
        }
        let lower = entry.to_ascii_lowercase();
        let (name, path, user_configured) = if let Some(alias) = alias_from_str(&lower) {
            (alias.as_str().to_string(), self.alias_path(alias), false)
        } else {
            let candidate = PathBuf::from(entry);
            let candidate = if candidate.is_absolute() {
                candidate
            } else {
                self.project_root.join(candidate)
            };
            (
                candidate
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or(entry)
                    .to_string(),
                candidate,
                true,
            )
        };
        if is_symlink_path(&path) {
            return Ok(None);
        }
        let path = normalize_path(path);
        if !path.is_dir() {
            return Ok(None);
        }
        if !self.is_allowed(&path, user_configured) {
            return Ok(None);
        }
        Ok(Some((name, path)))
    }

    fn alias_path(&self, alias: McpRootAlias) -> PathBuf {
        match alias {
            McpRootAlias::Root => self.project_root.clone(),
            McpRootAlias::Data => self.project_root.join("data"),
            McpRootAlias::Config => self.project_root.join("data").join("config"),
            McpRootAlias::Plugins => self.project_root.join("data").join("plugins"),
            McpRootAlias::PluginData => self.project_root.join("data").join("plugin_data"),
            McpRootAlias::Temp => self.project_root.join("data").join("temp"),
            McpRootAlias::Skills => self.project_root.join("data").join("skills"),
            McpRootAlias::KnowledgeBase => self.project_root.join("data").join("knowledge_base"),
            McpRootAlias::Backups => self.project_root.join("data").join("backups"),
        }
    }

    fn is_allowed(&self, path: &Path, user_configured: bool) -> bool {
        if user_configured && self.explicit_allowlist.contains(path) {
            return true;
        }
        if user_configured && self.explicit_allowlist.is_empty() {
            return false;
        }
        let allowed = McpRootAlias::all()
            .iter()
            .map(|alias| normalize_path(self.alias_path(*alias)))
            .chain(self.explicit_allowlist.iter().cloned())
            .collect::<Vec<_>>();
        allowed
            .iter()
            .any(|allowed_path| path == allowed_path || path.starts_with(allowed_path))
    }
}

fn alias_from_str(value: &str) -> Option<McpRootAlias> {
    McpRootAlias::all()
        .iter()
        .find(|alias| alias.as_str() == value)
        .copied()
}

fn normalize_path(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn is_symlink_path(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn path_to_file_uri(path: &Path) -> McpResult<McpUri> {
    let absolute = normalize_path(path.to_path_buf());
    let mut value = absolute.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        if !value.starts_with('/') {
            value = format!("/{value}");
        }
        value = value.replace(':', "%3A");
    }
    McpUri::new(format!("file://{value}")).map_err(|err| {
        McpError::Protocol(format!("failed to build MCP root file URI for path: {err}"))
    })
}
