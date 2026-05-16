use std::sync::{Arc, RwLock};

use astrbot_core::Result;
use async_trait::async_trait;

use crate::dependency::conflict::DependencyConflictReport;
use crate::dependency::environment::PluginImportEnvironment;

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

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn dependencies(&self) -> &[PluginDependency] {
        &self.dependencies
    }

    pub fn is_empty(&self) -> bool {
        self.dependencies.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyInstallRequest {
    pub plan: PluginDependencyPlan,
    pub environment: PluginImportEnvironment,
}

impl DependencyInstallRequest {
    pub fn new(plan: PluginDependencyPlan, environment: PluginImportEnvironment) -> Self {
        Self { plan, environment }
    }

    pub fn plugin_id(&self) -> &str {
        self.plan.plugin_id()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyInstallStatus {
    Completed,
    Skipped,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyInstallOutcome {
    pub plugin_id: String,
    pub status: DependencyInstallStatus,
    installed: Vec<PluginDependency>,
    skipped: Vec<PluginDependency>,
    conflicts: Vec<DependencyConflictReport>,
}

impl DependencyInstallOutcome {
    pub fn completed(plan: &PluginDependencyPlan) -> Self {
        Self {
            plugin_id: plan.plugin_id.clone(),
            status: DependencyInstallStatus::Completed,
            installed: plan.dependencies.clone(),
            skipped: Vec::new(),
            conflicts: Vec::new(),
        }
    }

    pub fn skipped(plan: &PluginDependencyPlan) -> Self {
        Self {
            plugin_id: plan.plugin_id.clone(),
            status: DependencyInstallStatus::Skipped,
            installed: Vec::new(),
            skipped: plan.dependencies.clone(),
            conflicts: Vec::new(),
        }
    }

    pub fn failed(plugin_id: impl Into<String>, conflicts: Vec<DependencyConflictReport>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            status: DependencyInstallStatus::Failed,
            installed: Vec::new(),
            skipped: Vec::new(),
            conflicts,
        }
    }

    pub fn installed(&self) -> &[PluginDependency] {
        &self.installed
    }

    pub fn skipped_dependencies(&self) -> &[PluginDependency] {
        &self.skipped
    }

    pub fn conflicts(&self) -> &[DependencyConflictReport] {
        &self.conflicts
    }

    pub fn is_success(&self) -> bool {
        !matches!(self.status, DependencyInstallStatus::Failed)
    }
}

#[async_trait]
pub trait PluginDependencyPlanInstaller: Send + Sync {
    async fn install_dependencies(
        &self,
        request: DependencyInstallRequest,
    ) -> Result<DependencyInstallOutcome>;
}

#[async_trait]
pub trait PluginDependencyInstaller: Send + Sync {
    async fn ensure_dependencies(&self, plan: &PluginDependencyPlan) -> Result<()>;
}

#[async_trait]
impl<T> PluginDependencyInstaller for T
where
    T: PluginDependencyPlanInstaller + Send + Sync,
{
    async fn ensure_dependencies(&self, plan: &PluginDependencyPlan) -> Result<()> {
        let request = DependencyInstallRequest::new(
            plan.clone(),
            PluginImportEnvironment::python_compat(plan.plugin_id()),
        );
        self.install_dependencies(request).await?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopDependencyInstaller;

#[async_trait]
impl PluginDependencyPlanInstaller for NoopDependencyInstaller {
    async fn install_dependencies(
        &self,
        request: DependencyInstallRequest,
    ) -> Result<DependencyInstallOutcome> {
        Ok(DependencyInstallOutcome::skipped(&request.plan))
    }
}

#[derive(Clone, Debug, Default)]
pub struct RecordingDependencyInstaller {
    requests: Arc<RwLock<Vec<DependencyInstallRequest>>>,
}

impl RecordingDependencyInstaller {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn requests(&self) -> Vec<DependencyInstallRequest> {
        self.requests
            .read()
            .expect("dependency installer requests lock")
            .clone()
    }
}

#[async_trait]
impl PluginDependencyPlanInstaller for RecordingDependencyInstaller {
    async fn install_dependencies(
        &self,
        request: DependencyInstallRequest,
    ) -> Result<DependencyInstallOutcome> {
        self.requests
            .write()
            .expect("dependency installer requests lock")
            .push(request.clone());
        Ok(DependencyInstallOutcome::completed(&request.plan))
    }
}
