mod adapters;
mod built;
mod core;
mod manager;
mod registry;

#[cfg(test)]
mod tests;

pub use adapters::{ConsolePlatform, ConsoleSink, MockPlatform, OneBotPlatform, WebChatPlatform};
pub use built::BuiltPlatform;
pub use core::{
    CONSOLE_PLATFORM_TYPE, MOCK_PLATFORM_TYPE, MessageRecorder, ONEBOT_PLATFORM_TYPE,
    PlatformAdapter, PlatformBuildContext, PlatformConfig, RecordingSink, SentMessage,
    StreamedMessage, WEBCHAT_PLATFORM_TYPE,
};
pub use manager::PlatformManager;
pub use registry::PlatformRegistry;

pub(crate) use core::validate_platform_id;
