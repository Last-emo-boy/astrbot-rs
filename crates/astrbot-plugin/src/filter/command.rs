use astrbot_core::MessageEvent;

use super::EventFilter;

#[derive(Clone, Debug)]
pub struct CommandFilter {
    command: String,
    aliases: Vec<String>,
    prefix: String,
}

impl CommandFilter {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: normalize_command(command),
            aliases: Vec::new(),
            prefix: "/".to_string(),
        }
    }

    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        let alias = normalize_command(alias);
        if !alias.is_empty() && !self.aliases.iter().any(|known| known == &alias) {
            self.aliases.push(alias);
        }
        self
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }

    fn command_names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.command.as_str()).chain(self.aliases.iter().map(String::as_str))
    }
}

impl EventFilter for CommandFilter {
    fn matches(&self, event: &MessageEvent) -> bool {
        let message = event.message.plain_text();
        let Some(command_text) = message.trim_start().strip_prefix(&self.prefix) else {
            return false;
        };
        let command_text = command_text.split_whitespace().next().unwrap_or_default();
        self.command_names().any(|name| name == command_text)
    }
}

fn normalize_command(command: impl Into<String>) -> String {
    command.into().trim().trim_start_matches('/').to_string()
}
