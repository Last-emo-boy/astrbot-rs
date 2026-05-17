mod adapter;
mod build_context;
mod config;
mod recording;
mod validation;

pub use adapter::{MessageRecorder, PlatformAdapter};
pub use build_context::PlatformBuildContext;
pub use config::{
    CONSOLE_PLATFORM_TYPE, MOCK_PLATFORM_TYPE, ONEBOT_PLATFORM_TYPE, PlatformConfig,
    WEBCHAT_PLATFORM_TYPE,
};
pub use recording::{RecordingSink, SentMessage, StreamedMessage};

pub(crate) use validation::validate_platform_id;
