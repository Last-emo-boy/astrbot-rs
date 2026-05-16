use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginFileChangeKind {
    Created,
    Modified,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginFileChange {
    pub plugin_id: String,
    pub path: PathBuf,
    pub kind: PluginFileChangeKind,
}

impl PluginFileChange {
    pub fn new(
        plugin_id: impl Into<String>,
        path: impl Into<PathBuf>,
        kind: PluginFileChangeKind,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            path: path.into(),
            kind,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotReloadDecision {
    Reload,
    Unload,
    Ignore,
}

pub fn plan_hot_reload(change: &PluginFileChange) -> HotReloadDecision {
    match change.kind {
        PluginFileChangeKind::Created | PluginFileChangeKind::Modified => {
            if is_plugin_source_file(&change.path) {
                HotReloadDecision::Reload
            } else {
                HotReloadDecision::Ignore
            }
        }
        PluginFileChangeKind::Removed => HotReloadDecision::Unload,
    }
}

fn is_plugin_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("rs" | "py" | "toml" | "yaml" | "yml" | "json")
    )
}
