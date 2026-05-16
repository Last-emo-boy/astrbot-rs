use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePathConfig {
    #[serde(default)]
    pub root_dir: Option<PathBuf>,
    #[serde(default)]
    pub data_dir: Option<PathBuf>,
    #[serde(default)]
    pub config_dir: Option<PathBuf>,
    #[serde(default)]
    pub plugin_dir: Option<PathBuf>,
    #[serde(default)]
    pub plugin_data_dir: Option<PathBuf>,
    #[serde(default)]
    pub temp_dir: Option<PathBuf>,
    #[serde(default)]
    pub attachment_dir: Option<PathBuf>,
    #[serde(default)]
    pub generated_media_dir: Option<PathBuf>,
    #[serde(default)]
    pub t2i_template_dir: Option<PathBuf>,
    #[serde(default)]
    pub webchat_dir: Option<PathBuf>,
    #[serde(default)]
    pub skills_dir: Option<PathBuf>,
    #[serde(default)]
    pub site_packages_dir: Option<PathBuf>,
    #[serde(default)]
    pub knowledge_base_dir: Option<PathBuf>,
    #[serde(default)]
    pub backups_dir: Option<PathBuf>,
}

impl Default for RuntimePathConfig {
    fn default() -> Self {
        Self {
            root_dir: Some(default_astrbot_root()),
            data_dir: None,
            config_dir: None,
            plugin_dir: None,
            plugin_data_dir: None,
            temp_dir: None,
            attachment_dir: None,
            generated_media_dir: None,
            t2i_template_dir: None,
            webchat_dir: None,
            skills_dir: None,
            site_packages_dir: None,
            knowledge_base_dir: None,
            backups_dir: None,
        }
    }
}

impl RuntimePathConfig {
    pub fn with_root_dir(mut self, root_dir: impl Into<PathBuf>) -> Self {
        self.root_dir = Some(root_dir.into());
        self
    }

    pub fn with_data_dir(mut self, data_dir: impl Into<PathBuf>) -> Self {
        self.data_dir = Some(data_dir.into());
        self
    }

    pub fn resolve(&self) -> RuntimePathLayout {
        let root_dir = normalize_path(self.root_dir.clone().unwrap_or_else(default_astrbot_root));
        let data_dir = normalize_path(
            self.data_dir
                .clone()
                .unwrap_or_else(|| root_dir.join("data")),
        );

        RuntimePathLayout {
            root_dir,
            config_dir: resolve_child(&data_dir, self.config_dir.as_deref(), "config"),
            plugin_dir: resolve_child(&data_dir, self.plugin_dir.as_deref(), "plugins"),
            plugin_data_dir: resolve_child(
                &data_dir,
                self.plugin_data_dir.as_deref(),
                "plugin_data",
            ),
            temp_dir: resolve_child(&data_dir, self.temp_dir.as_deref(), "temp"),
            attachment_dir: resolve_child(&data_dir, self.attachment_dir.as_deref(), "attachments"),
            generated_media_dir: resolve_child(
                &resolve_child(&data_dir, self.temp_dir.as_deref(), "temp"),
                self.generated_media_dir.as_deref(),
                "generated_media",
            ),
            t2i_template_dir: resolve_child(
                &data_dir,
                self.t2i_template_dir.as_deref(),
                "t2i_templates",
            ),
            webchat_dir: resolve_child(&data_dir, self.webchat_dir.as_deref(), "webchat"),
            skills_dir: resolve_child(&data_dir, self.skills_dir.as_deref(), "skills"),
            site_packages_dir: resolve_child(
                &data_dir,
                self.site_packages_dir.as_deref(),
                "site-packages",
            ),
            knowledge_base_dir: resolve_child(
                &data_dir,
                self.knowledge_base_dir.as_deref(),
                "knowledge_base",
            ),
            backups_dir: resolve_child(&data_dir, self.backups_dir.as_deref(), "backups"),
            data_dir,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePathLayout {
    pub root_dir: PathBuf,
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub plugin_dir: PathBuf,
    pub plugin_data_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub attachment_dir: PathBuf,
    pub generated_media_dir: PathBuf,
    pub t2i_template_dir: PathBuf,
    pub webchat_dir: PathBuf,
    pub skills_dir: PathBuf,
    pub site_packages_dir: PathBuf,
    pub knowledge_base_dir: PathBuf,
    pub backups_dir: PathBuf,
}

fn resolve_child(base: &Path, override_path: Option<&Path>, default_child: &str) -> PathBuf {
    let path = match override_path {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => base.join(path),
        None => base.join(default_child),
    };
    normalize_path(path)
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

    use super::RuntimePathConfig;

    #[test]
    fn resolves_astrbot_data_subdirectories_from_root() {
        let layout = RuntimePathConfig::default()
            .with_root_dir("workspace")
            .resolve();

        assert_eq!(layout.data_dir, PathBuf::from("workspace/data"));
        assert_eq!(layout.config_dir, PathBuf::from("workspace/data/config"));
        assert_eq!(layout.plugin_dir, PathBuf::from("workspace/data/plugins"));
        assert_eq!(
            layout.plugin_data_dir,
            PathBuf::from("workspace/data/plugin_data")
        );
        assert_eq!(layout.temp_dir, PathBuf::from("workspace/data/temp"));
        assert_eq!(
            layout.generated_media_dir,
            PathBuf::from("workspace/data/temp/generated_media")
        );
        assert_eq!(
            layout.t2i_template_dir,
            PathBuf::from("workspace/data/t2i_templates")
        );
        assert_eq!(layout.webchat_dir, PathBuf::from("workspace/data/webchat"));
        assert_eq!(layout.skills_dir, PathBuf::from("workspace/data/skills"));
        assert_eq!(
            layout.site_packages_dir,
            PathBuf::from("workspace/data/site-packages")
        );
        assert_eq!(
            layout.knowledge_base_dir,
            PathBuf::from("workspace/data/knowledge_base")
        );
        assert_eq!(layout.backups_dir, PathBuf::from("workspace/data/backups"));
    }

    #[test]
    fn relative_overrides_are_scoped_under_data_or_temp_roots() {
        let layout = RuntimePathConfig {
            temp_dir: Some(PathBuf::from("scratch")),
            generated_media_dir: Some(PathBuf::from("tts")),
            attachment_dir: Some(PathBuf::from("files")),
            ..RuntimePathConfig::default().with_root_dir("workspace")
        }
        .resolve();

        assert_eq!(layout.temp_dir, PathBuf::from("workspace/data/scratch"));
        assert_eq!(
            layout.generated_media_dir,
            PathBuf::from("workspace/data/scratch/tts")
        );
        assert_eq!(layout.attachment_dir, PathBuf::from("workspace/data/files"));
    }
}
