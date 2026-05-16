#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCapability {
    Shell,
    Python,
    Browser,
    Network,
    FileSystem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginPermission {
    SendMessage,
    AccessProvider,
    AccessPlatform,
    RegisterLlmTool,
    RegisterWebApi,
    SpawnBackgroundTask,
    UseNetwork,
    UseFileSystem,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxProfile {
    pub name: String,
    permissions: Vec<PluginPermission>,
    tool_capabilities: Vec<ToolCapability>,
}

impl Default for SandboxProfile {
    fn default() -> Self {
        Self::restricted()
    }
}

impl SandboxProfile {
    pub fn restricted() -> Self {
        Self {
            name: "restricted".to_string(),
            permissions: Vec::new(),
            tool_capabilities: Vec::new(),
        }
    }

    pub fn trusted() -> Self {
        Self {
            name: "trusted".to_string(),
            permissions: vec![
                PluginPermission::SendMessage,
                PluginPermission::AccessProvider,
                PluginPermission::AccessPlatform,
                PluginPermission::RegisterLlmTool,
                PluginPermission::RegisterWebApi,
                PluginPermission::SpawnBackgroundTask,
                PluginPermission::UseNetwork,
                PluginPermission::UseFileSystem,
            ],
            tool_capabilities: vec![
                ToolCapability::Shell,
                ToolCapability::Python,
                ToolCapability::Browser,
                ToolCapability::Network,
                ToolCapability::FileSystem,
            ],
        }
    }

    pub fn named(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        if !name.trim().is_empty() {
            self.name = name;
        }
        self
    }

    pub fn with_permission(mut self, permission: PluginPermission) -> Self {
        if !self.permissions.contains(&permission) {
            self.permissions.push(permission);
        }
        self
    }

    pub fn with_tool_capability(mut self, capability: ToolCapability) -> Self {
        if !self.tool_capabilities.contains(&capability) {
            self.tool_capabilities.push(capability);
        }
        self
    }

    pub fn permissions(&self) -> &[PluginPermission] {
        &self.permissions
    }

    pub fn tool_capabilities(&self) -> &[ToolCapability] {
        &self.tool_capabilities
    }

    pub fn allows_permission(&self, permission: PluginPermission) -> bool {
        self.permissions.contains(&permission)
    }

    pub fn allows_tool_capability(&self, capability: ToolCapability) -> bool {
        self.tool_capabilities.contains(&capability)
    }
}

pub trait SandboxRuntime: Send + Sync {
    fn profile_for(&self, plugin_name: &str, session_id: Option<&str>) -> SandboxProfile;

    fn allows_permission(
        &self,
        plugin_name: &str,
        session_id: Option<&str>,
        permission: PluginPermission,
    ) -> bool {
        self.profile_for(plugin_name, session_id)
            .allows_permission(permission)
    }

    fn allows_tool_capability(
        &self,
        plugin_name: &str,
        session_id: Option<&str>,
        capability: ToolCapability,
    ) -> bool {
        self.profile_for(plugin_name, session_id)
            .allows_tool_capability(capability)
    }
}
