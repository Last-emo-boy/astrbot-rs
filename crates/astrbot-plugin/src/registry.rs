use astrbot_core::{MessageEvent, Result};
use astrbot_tool::{
    CommandConflict, CommandDescriptor, CommandPermission, detect_command_conflicts,
};

use crate::event::{PluginControl, PluginEventType};
use crate::handler::RegisteredHandler;

#[derive(Clone, Default)]
pub struct PluginRegistry {
    handlers: Vec<RegisteredHandler>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_handler(&mut self, handler: RegisteredHandler) {
        self.handlers.push(handler);
        self.handlers
            .sort_by_key(|handler| -handler.metadata().priority);
    }

    pub async fn unregister_plugin(&mut self, plugin_name: &str) -> Result<usize> {
        let mut kept = Vec::with_capacity(self.handlers.len());
        let mut removed = 0;
        for handler in self.handlers.drain(..) {
            if handler.metadata().plugin_name == plugin_name {
                handler.terminate().await?;
                removed += 1;
            } else {
                kept.push(handler);
            }
        }
        self.handlers = kept;
        Ok(removed)
    }

    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn handlers(&self) -> &[RegisteredHandler] {
        &self.handlers
    }

    pub fn command_descriptors(&self) -> Vec<CommandDescriptor> {
        self.handlers
            .iter()
            .filter_map(command_descriptor_for_handler)
            .collect()
    }

    pub fn command_conflicts(&self) -> Vec<CommandConflict> {
        detect_command_conflicts(&self.command_descriptors())
    }

    pub async fn handle_event(
        &self,
        event_type: PluginEventType,
        event: &mut MessageEvent,
    ) -> Result<PluginControl> {
        for registered in &self.handlers {
            if !registered.matches(event_type, event) {
                continue;
            }

            let control = registered.handle(event).await?;
            if control == PluginControl::Stop || event.is_stopped() {
                return Ok(PluginControl::Stop);
            }
        }

        Ok(PluginControl::Continue)
    }

    pub async fn terminate(&self) -> Result<()> {
        for registered in &self.handlers {
            registered.terminate().await?;
        }
        Ok(())
    }
}

fn command_descriptor_for_handler(handler: &RegisteredHandler) -> Option<CommandDescriptor> {
    let command = handler
        .filters()
        .iter()
        .find_map(|filter| filter.command_metadata())?;
    let permission = handler
        .filters()
        .iter()
        .filter_map(|filter| filter.command_permission())
        .fold(CommandPermission::Everyone, strongest_permission);
    let metadata = handler.metadata();
    let mut descriptor = CommandDescriptor::new(
        format!("{}.{}", metadata.plugin_name, metadata.handler_name),
        metadata.plugin_name.clone(),
        command.command,
    )
    .with_command_type(command.command_type)
    .with_parent_signature(command.parent_signature)
    .with_permission(permission);
    for alias in command.aliases {
        descriptor = descriptor.with_alias(alias);
    }
    if let Some(description) = metadata.description.as_ref() {
        descriptor = descriptor.with_description(description.clone());
    }
    if !metadata.enabled {
        descriptor = descriptor.disabled();
    }
    if metadata.plugin_name.starts_with("builtin") {
        descriptor = descriptor.reserved();
    }
    Some(descriptor)
}

fn strongest_permission(left: CommandPermission, right: CommandPermission) -> CommandPermission {
    if command_permission_rank(left) >= command_permission_rank(right) {
        left
    } else {
        right
    }
}

fn command_permission_rank(permission: CommandPermission) -> u8 {
    match permission {
        CommandPermission::Everyone => 0,
        CommandPermission::Member => 1,
        CommandPermission::Admin => 2,
    }
}
