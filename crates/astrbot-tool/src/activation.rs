use std::collections::{BTreeMap, BTreeSet};

use crate::ToolDescriptor;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolActivationPolicy {
    disabled_tools: BTreeSet<String>,
    renames: BTreeMap<String, String>,
}

impl ToolActivationPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn disable(mut self, tool_name: impl Into<String>) -> Self {
        let tool_name = tool_name.into();
        if !tool_name.trim().is_empty() {
            self.disabled_tools.insert(tool_name);
        }
        self
    }

    pub fn enable(mut self, tool_name: impl AsRef<str>) -> Self {
        self.disabled_tools.remove(tool_name.as_ref());
        self
    }

    pub fn set_enabled(self, tool_name: impl Into<String>, enabled: bool) -> Self {
        if enabled {
            self.enable(tool_name.into())
        } else {
            self.disable(tool_name)
        }
    }

    pub fn rename(mut self, original: impl Into<String>, resolved: impl Into<String>) -> Self {
        let original = original.into();
        let resolved = resolved.into();
        if !original.trim().is_empty() && !resolved.trim().is_empty() {
            self.renames.insert(original, resolved);
        }
        self
    }

    pub fn is_enabled(&self, tool_name: &str) -> bool {
        !self.disabled_tools.contains(tool_name)
    }

    pub fn is_enabled_for(&self, tool: &ToolDescriptor) -> bool {
        !self.disabled_tools.contains(&tool.name) || !tool.source.allows_user_toggle()
    }

    pub fn rename_for(&self, tool_name: &str) -> Option<&str> {
        self.renames.get(tool_name).map(String::as_str)
    }
}
