use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{ToolActivationPolicy, ToolSource, ToolSourceMetadata};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_parameters")]
    pub parameters: Value,
    #[serde(default)]
    pub source: ToolSourceMetadata,
    #[serde(default = "default_active")]
    pub active: bool,
}

impl ToolDescriptor {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            parameters: default_parameters(),
            source: ToolSourceMetadata::new(ToolSource::Plugin),
            active: true,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.trim().is_empty()).then_some(description);
        self
    }

    pub fn with_parameters(mut self, parameters: Value) -> Self {
        self.parameters = parameters;
        self
    }

    pub fn with_source(mut self, source: ToolSource) -> Self {
        self.source = ToolSourceMetadata::new(source);
        self
    }

    pub fn with_source_metadata(mut self, source: ToolSourceMetadata) -> Self {
        self.source = source;
        self
    }

    pub fn inactive(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn apply_policy(&self, policy: &ToolActivationPolicy) -> Option<Self> {
        if !self.active || !policy.is_enabled_for(self) {
            return None;
        }

        let mut descriptor = self.clone();
        if let Some(rename) = policy.rename_for(&descriptor.name) {
            descriptor.name = rename.to_string();
        }
        Some(descriptor)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolCatalog {
    tools: Vec<ToolDescriptor>,
}

impl ToolCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_tool(&mut self, tool: ToolDescriptor) {
        if let Some(existing) = self.tools.iter_mut().find(|item| item.name == tool.name) {
            *existing = tool;
        } else {
            self.tools.push(tool);
        }
        self.tools.sort_by(|left, right| left.name.cmp(&right.name));
    }

    pub fn remove_tool(&mut self, name: &str) {
        self.tools.retain(|tool| tool.name != name);
    }

    pub fn remove_tools_by_plugin(&mut self, plugin_id: &str) -> Vec<ToolDescriptor> {
        let mut removed = Vec::new();
        self.tools.retain(|tool| {
            if tool.source.plugin_id.as_deref() == Some(plugin_id) {
                removed.push(tool.clone());
                false
            } else {
                true
            }
        });
        removed
    }

    pub fn tool(&self, name: &str) -> Option<&ToolDescriptor> {
        self.tools.iter().find(|tool| tool.name == name)
    }

    pub fn tools(&self) -> &[ToolDescriptor] {
        &self.tools
    }

    pub fn active_tools(&self, policy: &ToolActivationPolicy) -> Vec<ToolDescriptor> {
        self.tools
            .iter()
            .filter_map(|tool| tool.apply_policy(policy))
            .collect()
    }
}

fn default_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {}
    })
}

fn default_active() -> bool {
    true
}
