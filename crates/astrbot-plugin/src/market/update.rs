use serde::{Deserialize, Serialize};

use super::{PluginCompatibility, PluginInstallSource, PluginMarketEntry, PluginPackageDescriptor};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginMarketAction {
    Install,
    Update,
    Uninstall,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMarketOperationPlan {
    pub plugin_id: String,
    pub action: PluginMarketAction,
    pub package: Option<PluginPackageDescriptor>,
    pub requires_download: bool,
    pub requires_unpack: bool,
    pub requires_loader_reload: bool,
    pub compatibility: PluginCompatibility,
    pub delete_config: bool,
    pub delete_data: bool,
}

impl PluginMarketOperationPlan {
    pub fn install(request: PluginInstallPlan) -> Self {
        Self {
            plugin_id: request.plugin_id,
            action: PluginMarketAction::Install,
            package: Some(request.package),
            requires_download: true,
            requires_unpack: true,
            requires_loader_reload: true,
            compatibility: request.compatibility,
            delete_config: false,
            delete_data: false,
        }
    }

    pub fn update(request: PluginUpdatePlan) -> Self {
        Self {
            plugin_id: request.plugin_id,
            action: PluginMarketAction::Update,
            package: Some(request.package),
            requires_download: true,
            requires_unpack: true,
            requires_loader_reload: true,
            compatibility: request.compatibility,
            delete_config: false,
            delete_data: false,
        }
    }

    pub fn uninstall(request: PluginUninstallPlan) -> Self {
        Self {
            plugin_id: request.plugin_id,
            action: PluginMarketAction::Uninstall,
            package: None,
            requires_download: false,
            requires_unpack: false,
            requires_loader_reload: true,
            compatibility: PluginCompatibility::unknown(),
            delete_config: request.delete_config,
            delete_data: request.delete_data,
        }
    }

    pub fn from_market_entry(entry: &PluginMarketEntry) -> Option<Self> {
        let package = entry.package.clone().or_else(|| {
            entry
                .repo_url
                .as_ref()
                .map(|url| PluginPackageDescriptor::new(PluginInstallSource::repository(url)))
        })?;

        Some(Self::install(PluginInstallPlan {
            plugin_id: entry.plugin_id.clone(),
            package,
            compatibility: entry.compatibility.clone(),
            ignore_compatibility: false,
        }))
    }

    pub fn is_blocked_by_compatibility(&self) -> bool {
        !self.compatibility.compatible
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInstallPlan {
    pub plugin_id: String,
    pub package: PluginPackageDescriptor,
    pub compatibility: PluginCompatibility,
    pub ignore_compatibility: bool,
}

impl PluginInstallPlan {
    pub fn new(plugin_id: impl Into<String>, package: PluginPackageDescriptor) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            package,
            compatibility: PluginCompatibility::unknown(),
            ignore_compatibility: false,
        }
    }

    pub fn with_compatibility(mut self, compatibility: PluginCompatibility) -> Self {
        self.compatibility = compatibility;
        self
    }

    pub fn ignore_compatibility(mut self) -> Self {
        self.ignore_compatibility = true;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginUpdatePlan {
    pub plugin_id: String,
    pub package: PluginPackageDescriptor,
    pub compatibility: PluginCompatibility,
}

impl PluginUpdatePlan {
    pub fn new(plugin_id: impl Into<String>, package: PluginPackageDescriptor) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            package,
            compatibility: PluginCompatibility::unknown(),
        }
    }

    pub fn with_compatibility(mut self, compatibility: PluginCompatibility) -> Self {
        self.compatibility = compatibility;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginUninstallPlan {
    pub plugin_id: String,
    pub delete_config: bool,
    pub delete_data: bool,
}

impl PluginUninstallPlan {
    pub fn new(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            delete_config: false,
            delete_data: false,
        }
    }

    pub fn delete_config(mut self) -> Self {
        self.delete_config = true;
        self
    }

    pub fn delete_data(mut self) -> Self {
        self.delete_data = true;
        self
    }
}
