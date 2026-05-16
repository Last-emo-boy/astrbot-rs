mod dependency;
mod hot_reload;
mod lifecycle;
mod metadata;
mod store;

use std::sync::Arc;

use astrbot_core::Result;

pub use dependency::{
    DependencyConflictKind, DependencyConflictReport, DependencyErrorRedactor,
    DependencyInstallOutcome, DependencyInstallRequest, DependencyInstallStatus,
    NoopDependencyInstaller, PackagePreferencePolicy, PluginDependency, PluginDependencyInstaller,
    PluginDependencyKind, PluginDependencyPlan, PluginDependencyPlanInstaller,
    PluginImportEnvironment, PluginRuntimeKind, RecordingDependencyInstaller,
    RuntimeImportBehavior,
};
pub use hot_reload::{HotReloadDecision, PluginFileChange, PluginFileChangeKind, plan_hot_reload};
pub use lifecycle::{PluginLifecycleAction, PluginLifecycleEvent, PluginLifecycleState};
pub use metadata::{PluginLoadSource, PluginLoadSourceKind, PluginMetadata};
pub use store::{InMemoryPluginStore, PluginRecord, PluginStateStore};

use crate::manifest::PluginManifest;
use crate::sdk::PluginContext;

pub struct PluginLoader<S = InMemoryPluginStore> {
    store: S,
    dependency_installer: Arc<dyn PluginDependencyPlanInstaller>,
}

impl PluginLoader<InMemoryPluginStore> {
    pub fn new() -> Self {
        Self::with_store(InMemoryPluginStore::new())
    }
}

impl Default for PluginLoader<InMemoryPluginStore> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> PluginLoader<S>
where
    S: PluginStateStore,
{
    pub fn with_store(store: S) -> Self {
        Self {
            store,
            dependency_installer: Arc::new(NoopDependencyInstaller),
        }
    }

    pub fn with_dependency_installer<I>(mut self, dependency_installer: I) -> Self
    where
        I: PluginDependencyPlanInstaller + 'static,
    {
        self.dependency_installer = Arc::new(dependency_installer);
        self
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    pub fn dependency_installer(&self) -> &dyn PluginDependencyPlanInstaller {
        self.dependency_installer.as_ref()
    }

    pub fn discover_manifest(
        &mut self,
        source: PluginLoadSource,
        manifest: PluginManifest,
    ) -> Result<PluginMetadata> {
        let metadata = PluginMetadata::from_manifest(source, manifest)?;
        let record = PluginRecord::new(metadata.clone(), PluginLifecycleState::Discovered);
        self.store.upsert(record);
        Ok(metadata)
    }

    pub fn mark_loaded(&mut self, plugin_id: &str) -> Result<PluginLifecycleEvent> {
        self.transition(plugin_id, PluginLifecycleAction::Load)
    }

    pub fn activate(&mut self, plugin_id: &str) -> Result<PluginLifecycleEvent> {
        self.transition(plugin_id, PluginLifecycleAction::Activate)
    }

    pub fn disable(&mut self, plugin_id: &str) -> Result<PluginLifecycleEvent> {
        self.transition(plugin_id, PluginLifecycleAction::Disable)
    }

    pub fn unload(&mut self, plugin_id: &str) -> Result<PluginLifecycleEvent> {
        self.transition(plugin_id, PluginLifecycleAction::Unload)
    }

    pub fn context_for(&self, plugin_id: &str) -> Option<PluginContext> {
        self.store()
            .get(plugin_id)
            .map(|record| PluginContext::from_manifest(&record.metadata.manifest))
    }

    pub async fn ensure_dependencies(
        &self,
        plan: PluginDependencyPlan,
        environment: PluginImportEnvironment,
    ) -> Result<DependencyInstallOutcome> {
        self.dependency_installer
            .install_dependencies(DependencyInstallRequest::new(plan, environment))
            .await
    }

    fn transition(
        &mut self,
        plugin_id: &str,
        action: PluginLifecycleAction,
    ) -> Result<PluginLifecycleEvent> {
        let record = self.store.get(plugin_id).cloned().ok_or_else(|| {
            astrbot_core::AstrbotError::Pipeline(format!("plugin {plugin_id} is not discovered"))
        })?;
        let previous = record.state;
        let next = action.next_state(previous);
        self.store.set_state(plugin_id, next)?;
        Ok(PluginLifecycleEvent::new(
            record.metadata.plugin_id().to_string(),
            action,
            previous,
            next,
        ))
    }
}
