use std::path::PathBuf;

use astrbot_core::{AstrbotError, Result};

use crate::manifest::PluginManifest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginLoadSourceKind {
    NativeRust,
    PythonCompat,
    Wasm,
    ExternalProcess,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginLoadSource {
    plugin_id: String,
    kind: PluginLoadSourceKind,
    root_dir: Option<PathBuf>,
    module_path: Option<String>,
    reserved: bool,
}

impl PluginLoadSource {
    pub fn new(kind: PluginLoadSourceKind, plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: normalize_plugin_id(&plugin_id.into()),
            kind,
            root_dir: None,
            module_path: None,
            reserved: false,
        }
    }

    pub fn native_rust(plugin_id: impl Into<String>) -> Self {
        Self::new(PluginLoadSourceKind::NativeRust, plugin_id)
    }

    pub fn python_compat(plugin_id: impl Into<String>) -> Self {
        Self::new(PluginLoadSourceKind::PythonCompat, plugin_id)
    }

    pub fn with_root_dir(mut self, root_dir: impl Into<PathBuf>) -> Self {
        self.root_dir = Some(root_dir.into());
        self
    }

    pub fn with_module_path(mut self, module_path: impl Into<String>) -> Self {
        let module_path = module_path.into();
        self.module_path = (!module_path.trim().is_empty()).then_some(module_path);
        self
    }

    pub fn reserved(mut self) -> Self {
        self.reserved = true;
        self
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn kind(&self) -> PluginLoadSourceKind {
        self.kind
    }

    pub fn root_dir(&self) -> Option<&PathBuf> {
        self.root_dir.as_ref()
    }

    pub fn module_path(&self) -> Option<&str> {
        self.module_path.as_deref()
    }

    pub fn is_reserved(&self) -> bool {
        self.reserved
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginMetadata {
    pub manifest: PluginManifest,
    pub source: PluginLoadSource,
    supported_platforms: Vec<String>,
    runtime_version: Option<String>,
}

impl PluginMetadata {
    pub fn from_manifest(mut source: PluginLoadSource, manifest: PluginManifest) -> Result<Self> {
        if manifest.name.trim().is_empty() {
            return Err(AstrbotError::Pipeline(
                "plugin manifest name cannot be empty".to_string(),
            ));
        }
        if source.plugin_id == "plugin" {
            source.plugin_id = normalize_plugin_id(&manifest.name);
        }

        Ok(Self {
            manifest,
            source,
            supported_platforms: Vec::new(),
            runtime_version: None,
        })
    }

    pub fn with_supported_platform(mut self, platform: impl Into<String>) -> Self {
        push_unique_normalized(&mut self.supported_platforms, platform);
        self
    }

    pub fn with_runtime_version(mut self, runtime_version: impl Into<String>) -> Self {
        let runtime_version = runtime_version.into();
        self.runtime_version = (!runtime_version.trim().is_empty()).then_some(runtime_version);
        self
    }

    pub fn plugin_id(&self) -> &str {
        self.source.plugin_id()
    }

    pub fn supported_platforms(&self) -> &[String] {
        &self.supported_platforms
    }

    pub fn runtime_version(&self) -> Option<&str> {
        self.runtime_version.as_deref()
    }
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

fn push_unique_normalized(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into().trim().to_string();
    if !value.is_empty() && !values.iter().any(|known| known == &value) {
        values.push(value);
    }
}
