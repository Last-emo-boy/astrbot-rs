mod command;
mod event_type;
mod permission;
mod platform;
mod regex;

use astrbot_core::MessageEvent;

pub use command::CommandFilter;
pub use event_type::MessageSessionKindFilter;
pub use permission::{PermissionFilter, PermissionLevel, PermissionResolver, PermissionScope};
pub use platform::PlatformFilter;
pub use regex::RegexFilter;

pub trait EventFilter: Send + Sync {
    fn matches(&self, event: &MessageEvent) -> bool;
}

#[derive(Clone, Debug, Default)]
pub struct AlwaysFilter;

impl EventFilter for AlwaysFilter {
    fn matches(&self, _event: &MessageEvent) -> bool {
        true
    }
}
