use crate::sandbox::{PluginPermission, ToolCapability};

use super::background::BackgroundTaskPolicy;
use super::handoff::HandoffToolTarget;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginToolKind {
    Local,
    Mcp { server: Option<String> },
    Handoff(HandoffToolTarget),
    Background(BackgroundTaskPolicy),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginToolDeclaration {
    pub name: String,
    pub description: Option<String>,
    pub kind: PluginToolKind,
    required_permissions: Vec<PluginPermission>,
    required_capabilities: Vec<ToolCapability>,
}

impl PluginToolDeclaration {
    pub fn local(name: impl Into<String>) -> Self {
        Self::new(name, PluginToolKind::Local)
    }

    pub fn mcp(name: impl Into<String>, server: impl Into<String>) -> Self {
        let server = server.into();
        Self::new(
            name,
            PluginToolKind::Mcp {
                server: (!server.trim().is_empty()).then_some(server),
            },
        )
    }

    pub fn handoff(name: impl Into<String>, target: HandoffToolTarget) -> Self {
        Self::new(name, PluginToolKind::Handoff(target))
    }

    pub fn background(name: impl Into<String>, policy: BackgroundTaskPolicy) -> Self {
        Self::new(name, PluginToolKind::Background(policy))
    }

    pub fn new(name: impl Into<String>, kind: PluginToolKind) -> Self {
        Self {
            name: name.into(),
            description: None,
            kind,
            required_permissions: Vec::new(),
            required_capabilities: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.trim().is_empty()).then_some(description);
        self
    }

    pub fn requires_permission(mut self, permission: PluginPermission) -> Self {
        if !self.required_permissions.contains(&permission) {
            self.required_permissions.push(permission);
        }
        self
    }

    pub fn requires_capability(mut self, capability: ToolCapability) -> Self {
        if !self.required_capabilities.contains(&capability) {
            self.required_capabilities.push(capability);
        }
        self
    }

    pub fn required_permissions(&self) -> &[PluginPermission] {
        &self.required_permissions
    }

    pub fn required_capabilities(&self) -> &[ToolCapability] {
        &self.required_capabilities
    }
}
