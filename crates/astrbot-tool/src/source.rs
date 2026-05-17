use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSource {
    #[default]
    Plugin,
    Internal,
    Mcp,
    Subagent,
    Handoff,
    Background,
}

impl ToolSource {
    pub fn normalized(self) -> Self {
        match self {
            Self::Handoff => Self::Subagent,
            source => source,
        }
    }

    pub fn source_label(self) -> &'static str {
        match self.normalized() {
            Self::Plugin | Self::Background => "plugin",
            Self::Internal => "internal",
            Self::Mcp => "mcp",
            Self::Subagent | Self::Handoff => "subagent",
        }
    }

    fn default_origin_name(self) -> Option<&'static str> {
        match self.normalized() {
            Self::Internal => Some("AstrBot"),
            _ => None,
        }
    }

    fn default_toggle_policy(self) -> ToolUserTogglePolicy {
        match self.normalized() {
            Self::Internal => ToolUserTogglePolicy::Denied,
            _ => ToolUserTogglePolicy::Allowed,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolUserTogglePolicy {
    #[default]
    Allowed,
    Denied,
}

impl ToolUserTogglePolicy {
    pub fn allows_user_toggle(self) -> bool {
        matches!(self, Self::Allowed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSourceMetadata {
    pub kind: ToolSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_server_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagent_id: Option<String>,
    #[serde(default)]
    pub user_toggle_policy: ToolUserTogglePolicy,
}

impl Default for ToolSourceMetadata {
    fn default() -> Self {
        Self::new(ToolSource::Plugin)
    }
}

impl ToolSourceMetadata {
    pub fn new(kind: ToolSource) -> Self {
        let kind = kind.normalized();
        Self {
            kind,
            origin: Some(kind.source_label().to_string()),
            origin_name: kind.default_origin_name().map(str::to_string),
            provider_id: None,
            plugin_id: None,
            mcp_server_name: None,
            subagent_id: None,
            user_toggle_policy: kind.default_toggle_policy(),
        }
    }

    pub fn plugin(plugin_id: impl Into<String>, origin_name: impl Into<String>) -> Self {
        Self::new(ToolSource::Plugin)
            .with_plugin_id(plugin_id)
            .with_origin_name(origin_name)
    }

    pub fn internal(origin_name: impl Into<String>) -> Self {
        Self::new(ToolSource::Internal).with_origin_name(origin_name)
    }

    pub fn internal_provider(
        provider_id: impl Into<String>,
        origin_name: impl Into<String>,
    ) -> Self {
        Self::internal(origin_name).with_provider_id(provider_id)
    }

    pub fn mcp(server_name: impl Into<String>) -> Self {
        Self::new(ToolSource::Mcp)
            .with_mcp_server_name(server_name)
            .with_origin_name_from_mcp_server()
    }

    pub fn subagent(subagent_id: impl Into<String>) -> Self {
        Self::new(ToolSource::Subagent)
            .with_subagent_id(subagent_id)
            .with_origin_name_from_subagent()
    }

    pub fn background(plugin_id: impl Into<String>, origin_name: impl Into<String>) -> Self {
        Self::new(ToolSource::Background)
            .with_plugin_id(plugin_id)
            .with_origin_name(origin_name)
    }

    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        self.origin = non_empty(origin.into());
        self
    }

    pub fn with_origin_name(mut self, origin_name: impl Into<String>) -> Self {
        self.origin_name = non_empty(origin_name.into());
        self
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = non_empty(provider_id.into());
        self
    }

    pub fn with_plugin_id(mut self, plugin_id: impl Into<String>) -> Self {
        self.plugin_id = non_empty(plugin_id.into());
        self
    }

    pub fn with_mcp_server_name(mut self, mcp_server_name: impl Into<String>) -> Self {
        self.mcp_server_name = non_empty(mcp_server_name.into());
        self
    }

    pub fn with_subagent_id(mut self, subagent_id: impl Into<String>) -> Self {
        self.subagent_id = non_empty(subagent_id.into());
        self
    }

    pub fn with_user_toggle_policy(mut self, policy: ToolUserTogglePolicy) -> Self {
        self.user_toggle_policy = policy;
        self
    }

    pub fn allow_user_toggle(self) -> Self {
        self.with_user_toggle_policy(ToolUserTogglePolicy::Allowed)
    }

    pub fn deny_user_toggle(self) -> Self {
        self.with_user_toggle_policy(ToolUserTogglePolicy::Denied)
    }

    pub fn allows_user_toggle(&self) -> bool {
        self.user_toggle_policy.allows_user_toggle()
    }

    pub fn origin(&self) -> &str {
        self.origin
            .as_deref()
            .unwrap_or_else(|| self.kind.source_label())
    }

    pub fn origin_name(&self) -> &str {
        self.origin_name
            .as_deref()
            .or_else(|| self.kind.default_origin_name())
            .unwrap_or_else(|| self.origin())
    }

    pub fn source_label(&self) -> &'static str {
        self.kind.source_label()
    }

    fn with_origin_name_from_mcp_server(mut self) -> Self {
        if self.origin_name.is_none() {
            self.origin_name = self.mcp_server_name.clone();
        }
        self
    }

    fn with_origin_name_from_subagent(mut self) -> Self {
        if self.origin_name.is_none() {
            self.origin_name = self.subagent_id.clone();
        }
        self
    }
}

impl PartialEq<ToolSource> for ToolSourceMetadata {
    fn eq(&self, other: &ToolSource) -> bool {
        self.kind == other.normalized()
    }
}

impl PartialEq<ToolSourceMetadata> for ToolSource {
    fn eq(&self, other: &ToolSourceMetadata) -> bool {
        self.normalized() == other.kind
    }
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}
