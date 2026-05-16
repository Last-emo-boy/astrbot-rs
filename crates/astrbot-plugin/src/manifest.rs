use crate::sandbox::{PluginPermission, ToolCapability};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginCapability {
    EventHandler,
    LlmTool,
    WebApi,
    ProviderAccess,
    PlatformAccess,
    BackgroundTask,
    SandboxTool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub capabilities: Vec<PluginCapability>,
    pub permissions: Vec<PluginPermission>,
    pub tool_capabilities: Vec<ToolCapability>,
}

impl PluginManifest {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            description: None,
            authors: Vec::new(),
            capabilities: Vec::new(),
            permissions: Vec::new(),
            tool_capabilities: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.trim().is_empty()).then_some(description);
        self
    }

    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        push_unique_normalized(&mut self.authors, author);
        self
    }

    pub fn with_capability(mut self, capability: PluginCapability) -> Self {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
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
}

fn push_unique_normalized(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into().trim().to_string();
    if !value.is_empty() && !values.iter().any(|known| known == &value) {
        values.push(value);
    }
}
