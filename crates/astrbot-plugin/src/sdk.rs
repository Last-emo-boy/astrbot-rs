use async_trait::async_trait;

use astrbot_core::Result;

use crate::extension::{PluginPlatformExtension, PluginWebApiRoute};
use crate::handler::RegisteredHandler;
use crate::loader::PluginDependencyPlan;
use crate::manifest::PluginManifest;
use crate::sandbox::{PluginPermission, SandboxProfile, ToolCapability};
use crate::tool::PluginToolDeclaration;

pub const PLUGIN_SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginContext {
    plugin_name: String,
    session_id: Option<String>,
    sandbox_profile: SandboxProfile,
}

impl PluginContext {
    pub fn new(plugin_name: impl Into<String>) -> Self {
        Self {
            plugin_name: plugin_name.into(),
            session_id: None,
            sandbox_profile: SandboxProfile::restricted(),
        }
    }

    pub fn from_manifest(manifest: &PluginManifest) -> Self {
        let mut profile = SandboxProfile::restricted();
        for permission in &manifest.permissions {
            profile = profile.with_permission(*permission);
        }
        for capability in &manifest.tool_capabilities {
            profile = profile.with_tool_capability(*capability);
        }

        Self {
            plugin_name: manifest.name.clone(),
            session_id: None,
            sandbox_profile: profile,
        }
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        self.session_id = (!session_id.trim().is_empty()).then_some(session_id);
        self
    }

    pub fn with_sandbox_profile(mut self, sandbox_profile: SandboxProfile) -> Self {
        self.sandbox_profile = sandbox_profile;
        self
    }

    pub fn plugin_name(&self) -> &str {
        &self.plugin_name
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn sandbox_profile(&self) -> &SandboxProfile {
        &self.sandbox_profile
    }

    pub fn allows_permission(&self, permission: PluginPermission) -> bool {
        self.sandbox_profile.allows_permission(permission)
    }

    pub fn allows_tool_capability(&self, capability: ToolCapability) -> bool {
        self.sandbox_profile.allows_tool_capability(capability)
    }
}

#[async_trait]
pub trait PluginModule: Send + Sync {
    fn manifest(&self) -> &PluginManifest;

    fn runtime_version_requirement(&self) -> Option<&str> {
        None
    }

    fn dependency_plan(&self, plugin_id: &str) -> PluginDependencyPlan {
        PluginDependencyPlan::new(plugin_id)
    }

    fn handlers(&self, _ctx: &PluginContext) -> Vec<RegisteredHandler> {
        Vec::new()
    }

    fn tools(&self, _ctx: &PluginContext) -> Vec<PluginToolDeclaration> {
        Vec::new()
    }

    fn web_routes(&self, _ctx: &PluginContext) -> Vec<PluginWebApiRoute> {
        Vec::new()
    }

    fn platform_extensions(&self, _ctx: &PluginContext) -> Vec<PluginPlatformExtension> {
        Vec::new()
    }

    async fn on_load(&self, _ctx: &PluginContext) -> Result<()> {
        Ok(())
    }

    async fn on_unload(&self, _ctx: &PluginContext) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginTestHarness {
    context: PluginContext,
}

impl PluginTestHarness {
    pub fn new(plugin_name: impl Into<String>) -> Self {
        Self {
            context: PluginContext::new(plugin_name),
        }
    }

    pub fn from_manifest(manifest: &PluginManifest) -> Self {
        Self {
            context: PluginContext::from_manifest(manifest),
        }
    }

    pub fn with_context(context: PluginContext) -> Self {
        Self { context }
    }

    pub fn context(&self) -> &PluginContext {
        &self.context
    }
}
