use astrbot_core::MessageEvent;
use astrbot_tool::CommandType;

use super::{CommandFilterMetadata, EventFilter};

#[derive(Clone, Debug)]
pub struct CommandFilter {
    command: String,
    aliases: Vec<String>,
    prefix: String,
    parent_signature: String,
    command_type: CommandType,
}

impl CommandFilter {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: normalize_command(command),
            aliases: Vec::new(),
            prefix: "/".to_string(),
            parent_signature: String::new(),
            command_type: CommandType::Command,
        }
    }

    pub fn group(command: impl Into<String>) -> Self {
        Self::new(command).with_command_type(CommandType::Group)
    }

    pub fn sub_command(parent_signature: impl Into<String>, command: impl Into<String>) -> Self {
        Self::new(command)
            .with_parent_signature(parent_signature)
            .with_command_type(CommandType::SubCommand)
    }

    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        let alias = normalize_command(alias);
        if !alias.is_empty() && !self.aliases.iter().any(|known| known == &alias) {
            self.aliases.push(alias);
        }
        self
    }

    pub fn with_aliases(mut self, aliases: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for alias in aliases {
            self = self.with_alias(alias);
        }
        self
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    pub fn with_parent_signature(mut self, parent_signature: impl Into<String>) -> Self {
        self.parent_signature = normalize_command(parent_signature);
        self
    }

    pub fn with_command_type(mut self, command_type: CommandType) -> Self {
        self.command_type = command_type;
        self
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }

    pub fn parent_signature(&self) -> &str {
        &self.parent_signature
    }

    pub fn command_type(&self) -> CommandType {
        self.command_type
    }

    pub fn complete_command_names(&self) -> Vec<String> {
        std::iter::once(self.command.as_str())
            .chain(self.aliases.iter().map(String::as_str))
            .map(|fragment| compose_command(&self.parent_signature, fragment))
            .filter(|command| !command.trim().is_empty())
            .collect()
    }
}

impl EventFilter for CommandFilter {
    fn matches(&self, event: &MessageEvent) -> bool {
        let message = event.message.plain_text();
        let Some(command_text) = message.trim_start().strip_prefix(&self.prefix) else {
            return false;
        };
        let command_parts = command_text.split_whitespace().collect::<Vec<_>>();
        self.complete_command_names().iter().any(|name| {
            let expected_parts = name.split_whitespace().collect::<Vec<_>>();
            !expected_parts.is_empty()
                && command_parts.len() >= expected_parts.len()
                && command_parts[..expected_parts.len()] == expected_parts[..]
        })
    }

    fn command_metadata(&self) -> Option<CommandFilterMetadata> {
        Some(CommandFilterMetadata {
            command: self.command.clone(),
            aliases: self.aliases.clone(),
            parent_signature: self.parent_signature.clone(),
            command_type: self.command_type,
        })
    }
}

fn normalize_command(command: impl Into<String>) -> String {
    command.into().trim().trim_start_matches('/').to_string()
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
