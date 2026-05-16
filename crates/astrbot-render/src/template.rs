use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_TEMPLATE_NAME: &str = "base";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TemplateName(String);

impl TemplateName {
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        if !is_safe_template_name(&name) {
            return Err(AstrbotError::Pipeline(format!(
                "invalid T2I template name: {name}"
            )));
        }
        Ok(Self(name))
    }

    pub fn base() -> Self {
        Self(DEFAULT_TEMPLATE_NAME.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TemplateName {
    fn default() -> Self {
        Self::base()
    }
}

impl AsRef<str> for TemplateName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateSource {
    Builtin,
    User,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateDescriptor {
    pub name: TemplateName,
    pub source: TemplateSource,
    pub is_default: bool,
}

#[derive(Clone, Debug)]
pub struct TemplateCatalog {
    builtin_templates: BTreeMap<TemplateName, String>,
    user_template_dir: Option<PathBuf>,
}

impl TemplateCatalog {
    pub fn new(user_template_dir: impl Into<PathBuf>) -> Self {
        Self::with_builtin_templates(user_template_dir, default_builtin_templates())
    }

    pub fn without_user_dir() -> Self {
        Self {
            builtin_templates: default_builtin_templates(),
            user_template_dir: None,
        }
    }

    pub fn with_builtin_templates(
        user_template_dir: impl Into<PathBuf>,
        builtin_templates: BTreeMap<TemplateName, String>,
    ) -> Self {
        Self {
            builtin_templates,
            user_template_dir: Some(user_template_dir.into()),
        }
    }

    pub fn list_templates(&self) -> Result<Vec<TemplateDescriptor>> {
        let mut names = self
            .builtin_templates
            .keys()
            .cloned()
            .collect::<BTreeSet<TemplateName>>();
        if let Some(user_dir) = &self.user_template_dir {
            for name in user_template_names(user_dir)? {
                names.insert(name);
            }
        }

        names
            .into_iter()
            .map(|name| {
                let source = if self
                    .user_template_path(&name)
                    .is_some_and(|path| path.exists())
                {
                    TemplateSource::User
                } else {
                    TemplateSource::Builtin
                };
                let is_default = name.as_str() == DEFAULT_TEMPLATE_NAME;
                Ok(TemplateDescriptor {
                    name,
                    source,
                    is_default,
                })
            })
            .collect()
    }

    pub fn get_template(&self, name: &TemplateName) -> Result<String> {
        if let Some(path) = self.user_template_path(name)
            && path.exists()
        {
            return fs::read_to_string(&path).map_err(|err| {
                AstrbotError::Pipeline(format!("read T2I user template {}: {err}", path.display()))
            });
        }

        self.builtin_templates.get(name).cloned().ok_or_else(|| {
            AstrbotError::Pipeline(format!("missing T2I template: {}", name.as_str()))
        })
    }

    pub fn put_user_template(&self, name: &TemplateName, content: &str) -> Result<()> {
        let path = self.require_user_template_path(name)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                AstrbotError::Pipeline(format!(
                    "create T2I template directory {}: {err}",
                    parent.display()
                ))
            })?;
        }
        fs::write(&path, content).map_err(|err| {
            AstrbotError::Pipeline(format!("write T2I user template {}: {err}", path.display()))
        })
    }

    pub fn delete_user_template(&self, name: &TemplateName) -> Result<()> {
        let path = self.require_user_template_path(name)?;
        if !path.exists() {
            return Err(AstrbotError::Pipeline(format!(
                "missing T2I user template: {}",
                name.as_str()
            )));
        }
        fs::remove_file(&path).map_err(|err| {
            AstrbotError::Pipeline(format!(
                "delete T2I user template {}: {err}",
                path.display()
            ))
        })
    }

    fn require_user_template_path(&self, name: &TemplateName) -> Result<PathBuf> {
        self.user_template_path(name).ok_or_else(|| {
            AstrbotError::Pipeline("T2I user template directory is not configured".to_string())
        })
    }

    fn user_template_path(&self, name: &TemplateName) -> Option<PathBuf> {
        self.user_template_dir
            .as_ref()
            .map(|dir| dir.join(format!("{}.html", name.as_str())))
    }
}

fn default_builtin_templates() -> BTreeMap<TemplateName, String> {
    BTreeMap::from([
        (
            TemplateName::base(),
            r#"<!doctype html><html><body><main>{{ text }}</main><footer>{{ version }}</footer></body></html>"#
                .to_string(),
        ),
        (
            TemplateName::new("astrbot_powershell").expect("static template name is valid"),
            r#"<!doctype html><html><body><pre>{{ text }}</pre><footer>{{ version }}</footer></body></html>"#
                .to_string(),
        ),
    ])
}

fn user_template_names(dir: &Path) -> Result<Vec<TemplateName>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut names = Vec::new();
    for entry in fs::read_dir(dir).map_err(|err| {
        AstrbotError::Pipeline(format!(
            "read T2I template directory {}: {err}",
            dir.display()
        ))
    })? {
        let entry = entry.map_err(|err| {
            AstrbotError::Pipeline(format!("read T2I template directory entry: {err}"))
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("html") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
            && let Ok(name) = TemplateName::new(stem)
        {
            names.push(name);
        }
    }
    Ok(names)
}

fn is_safe_template_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && trimmed == name
        && trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{TemplateCatalog, TemplateName, TemplateSource};

    #[test]
    fn rejects_path_traversal_template_names() {
        assert!(TemplateName::new("../base").is_err());
        assert!(TemplateName::new("base/name").is_err());
        assert!(TemplateName::new(" base").is_err());
        assert!(TemplateName::new("base").is_ok());
    }

    #[test]
    fn user_template_overrides_builtin_template() {
        let temp_dir =
            std::env::temp_dir().join(format!("astrbot_render_template_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let catalog = TemplateCatalog::new(&temp_dir);
        let base = TemplateName::base();
        catalog
            .put_user_template(&base, "custom {{ text }}")
            .unwrap();

        assert_eq!(catalog.get_template(&base).unwrap(), "custom {{ text }}");
        let listed = catalog
            .list_templates()
            .unwrap()
            .into_iter()
            .find(|entry| entry.name == base)
            .unwrap();
        assert_eq!(listed.source, TemplateSource::User);

        catalog.delete_user_template(&base).unwrap();
        let fallback = catalog.get_template(&base).unwrap();
        assert!(fallback.contains("{{ text }}"));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
