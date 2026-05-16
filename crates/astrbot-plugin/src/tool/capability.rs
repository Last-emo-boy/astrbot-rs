use crate::sandbox::{PluginPermission, SandboxProfile, ToolCapability};

use super::declaration::PluginToolDeclaration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCapabilityDecision {
    pub allowed: bool,
    pub missing_permissions: Vec<PluginPermission>,
    pub missing_capabilities: Vec<ToolCapability>,
}

impl ToolCapabilityDecision {
    pub fn check(declaration: &PluginToolDeclaration, profile: &SandboxProfile) -> Self {
        let missing_permissions = declaration
            .required_permissions()
            .iter()
            .copied()
            .filter(|permission| !profile.allows_permission(*permission))
            .collect::<Vec<_>>();
        let missing_capabilities = declaration
            .required_capabilities()
            .iter()
            .copied()
            .filter(|capability| !profile.allows_tool_capability(*capability))
            .collect::<Vec<_>>();

        Self {
            allowed: missing_permissions.is_empty() && missing_capabilities.is_empty(),
            missing_permissions,
            missing_capabilities,
        }
    }

    pub fn rejection_message(&self, tool_name: &str) -> Option<String> {
        if self.allowed {
            return None;
        }

        let mut parts = Vec::new();
        if !self.missing_permissions.is_empty() {
            parts.push(format!(
                "missing permissions: {:?}",
                self.missing_permissions
            ));
        }
        if !self.missing_capabilities.is_empty() {
            parts.push(format!(
                "missing tool capabilities: {:?}",
                self.missing_capabilities
            ));
        }
        Some(format!(
            "tool {tool_name} rejected by sandbox: {}",
            parts.join(", ")
        ))
    }
}
