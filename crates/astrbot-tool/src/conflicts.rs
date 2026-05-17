use std::collections::BTreeMap;

use crate::{CommandDescriptor, ToolDescriptor};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolConflict {
    pub tool_name: String,
    pub sources: Vec<String>,
}

pub fn detect_tool_conflicts(tools: &[ToolDescriptor]) -> Vec<ToolConflict> {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for tool in tools.iter().filter(|tool| tool.active) {
        grouped
            .entry(tool.name.clone())
            .or_default()
            .push(tool.source.source_label().to_string());
    }

    grouped
        .into_iter()
        .filter_map(|(tool_name, sources)| {
            (sources.len() > 1).then_some(ToolConflict { tool_name, sources })
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandConflict {
    pub command: String,
    pub handlers: Vec<String>,
}

pub fn detect_command_conflicts(commands: &[CommandDescriptor]) -> Vec<CommandConflict> {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for command in commands.iter().filter(|command| command.enabled) {
        for effective_command in std::iter::once(command.effective_command())
            .chain(command.effective_aliases().into_iter())
            .filter(|command| !command.trim().is_empty())
        {
            grouped
                .entry(effective_command)
                .or_default()
                .push(command.handler_full_name.clone());
        }
    }

    grouped
        .into_iter()
        .filter_map(|(command, handlers)| {
            (handlers.len() > 1).then_some(CommandConflict { command, handlers })
        })
        .collect()
}
