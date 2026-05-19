mod command;
mod event_type;
mod permission;
mod platform;
mod regex;

use astrbot_core::MessageEvent;
use astrbot_tool::{CommandPermission, CommandType};

pub use command::CommandFilter;
pub use event_type::MessageSessionKindFilter;
pub use permission::{PermissionFilter, PermissionLevel, PermissionResolver, PermissionScope};
pub use platform::PlatformFilter;
pub use regex::RegexFilter;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandFilterMetadata {
    pub command: String,
    pub aliases: Vec<String>,
    pub parent_signature: String,
    pub command_type: CommandType,
}

pub trait EventFilter: Send + Sync {
    fn matches(&self, event: &MessageEvent) -> bool;

    fn command_metadata(&self) -> Option<CommandFilterMetadata> {
        None
    }

    fn command_permission(&self) -> Option<CommandPermission> {
        None
    }
}

#[derive(Clone, Debug, Default)]
pub struct AlwaysFilter;

impl EventFilter for AlwaysFilter {
    fn matches(&self, _event: &MessageEvent) -> bool {
        true
    }
}
