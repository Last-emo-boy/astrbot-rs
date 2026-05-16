use astrbot_plugin::PluginRegistry;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManagementResponse {
    pub handler_count: usize,
    pub handlers: Vec<PluginHandlerManagementResponse>,
}

impl PluginManagementResponse {
    pub fn from_registry(registry: &PluginRegistry) -> Self {
        Self {
            handler_count: registry.handler_count(),
            handlers: registry
                .handlers()
                .iter()
                .map(|handler| {
                    let metadata = handler.metadata();
                    PluginHandlerManagementResponse {
                        plugin_name: metadata.plugin_name.clone(),
                        handler_name: metadata.handler_name.clone(),
                        event_type: format!("{:?}", metadata.event_type),
                        priority: metadata.priority,
                        enabled: metadata.enabled,
                    }
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginHandlerManagementResponse {
    pub plugin_name: String,
    pub handler_name: String,
    pub event_type: String,
    pub priority: i32,
    pub enabled: bool,
}
