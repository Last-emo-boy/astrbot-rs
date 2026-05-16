#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginPlatformExtensionKind {
    Adapter,
    MessageBridge,
    EventBridge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginPlatformExtension {
    pub plugin_id: String,
    pub extension_id: String,
    pub kind: PluginPlatformExtensionKind,
    pub platform_type: String,
    pub description: Option<String>,
}

impl PluginPlatformExtension {
    pub fn new(
        plugin_id: impl Into<String>,
        extension_id: impl Into<String>,
        kind: PluginPlatformExtensionKind,
        platform_type: impl Into<String>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            extension_id: extension_id.into(),
            kind,
            platform_type: platform_type.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.trim().is_empty()).then_some(description);
        self
    }
}
