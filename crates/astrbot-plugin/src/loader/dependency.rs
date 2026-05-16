use async_trait::async_trait;

use astrbot_core::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginDependencyKind {
    RustCrate,
    PythonPackage,
    SystemPackage,
    Binary,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginDependency {
    pub kind: PluginDependencyKind,
    pub name: String,
    pub version_req: Option<String>,
    pub optional: bool,
}

impl PluginDependency {
    pub fn new(kind: PluginDependencyKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            version_req: None,
            optional: false,
        }
    }

    pub fn with_version_req(mut self, version_req: impl Into<String>) -> Self {
        let version_req = version_req.into();
        self.version_req = (!version_req.trim().is_empty()).then_some(version_req);
        self
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginDependencyPlan {
    pub plugin_id: String,
    dependencies: Vec<PluginDependency>,
}

impl PluginDependencyPlan {
    pub fn new(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            dependencies: Vec::new(),
        }
    }

    pub fn with_dependency(mut self, dependency: PluginDependency) -> Self {
        if !dependency.name.trim().is_empty() {
            self.dependencies.push(dependency);
        }
        self
    }

    pub fn dependencies(&self) -> &[PluginDependency] {
        &self.dependencies
    }
}

#[async_trait]
pub trait PluginDependencyInstaller: Send + Sync {
    async fn ensure_dependencies(&self, plan: &PluginDependencyPlan) -> Result<()>;
}

pub struct NoopDependencyInstaller;

#[async_trait]
impl PluginDependencyInstaller for NoopDependencyInstaller {
    async fn ensure_dependencies(&self, _plan: &PluginDependencyPlan) -> Result<()> {
        Ok(())
    }
}
