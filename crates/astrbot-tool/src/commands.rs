use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandDescriptor {
    pub handler_full_name: String,
    pub plugin_name: String,
    pub description: String,
    pub command_type: CommandType,
    pub original_command: String,
    pub current_fragment: String,
    pub parent_signature: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub permission: CommandPermission,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub reserved: bool,
}

impl CommandDescriptor {
    pub fn new(
        handler_full_name: impl Into<String>,
        plugin_name: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        let command = command.into();
        Self {
            handler_full_name: handler_full_name.into(),
            plugin_name: plugin_name.into(),
            description: String::new(),
            command_type: CommandType::Command,
            original_command: command.clone(),
            current_fragment: command,
            parent_signature: String::new(),
            aliases: Vec::new(),
            permission: CommandPermission::Everyone,
            enabled: true,
            reserved: false,
        }
    }

    pub fn with_parent_signature(mut self, parent_signature: impl Into<String>) -> Self {
        self.parent_signature = parent_signature.into();
        self
    }

    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        let alias = alias.into();
        if !alias.trim().is_empty() {
            self.aliases.push(alias);
            self.aliases.sort();
            self.aliases.dedup();
        }
        self
    }

    pub fn with_permission(mut self, permission: CommandPermission) -> Self {
        self.permission = permission;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn effective_command(&self) -> String {
        compose_command(&self.parent_signature, &self.current_fragment)
    }

    pub fn effective_aliases(&self) -> Vec<String> {
        self.aliases
            .iter()
            .map(|alias| compose_command(&self.parent_signature, alias))
            .filter(|alias| !alias.trim().is_empty())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandType {
    #[default]
    Command,
    Group,
    SubCommand,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandPermission {
    #[default]
    Everyone,
    Member,
    Admin,
}

fn compose_command(parent_signature: &str, fragment: &str) -> String {
    let parent_signature = parent_signature.trim();
    let fragment = fragment.trim();

    match (parent_signature.is_empty(), fragment.is_empty()) {
        (true, true) => String::new(),
        (true, false) => fragment.to_string(),
        (false, true) => parent_signature.to_string(),
        (false, false) => format!("{parent_signature} {fragment}"),
    }
}

fn default_enabled() -> bool {
    true
}
