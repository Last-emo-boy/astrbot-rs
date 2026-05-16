use serde::{Deserialize, Serialize};

use crate::types::McpUri;

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
