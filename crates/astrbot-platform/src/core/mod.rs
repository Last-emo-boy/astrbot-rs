mod adapter;
mod build_context;
mod config;
mod recording;
mod validation;

pub use adapter::{MessageRecorder, PlatformAdapter};
pub use build_context::PlatformBuildContext;
pub use config::{
    AIOCQHTTP_PLATFORM_TYPE, CONSOLE_PLATFORM_TYPE, DINGTALK_PLATFORM_TYPE, DISCORD_PLATFORM_TYPE,
    KOOK_PLATFORM_TYPE, LARK_PLATFORM_TYPE, LINE_PLATFORM_TYPE, MISSKEY_PLATFORM_TYPE,
    MOCK_PLATFORM_TYPE, ONEBOT_PLATFORM_TYPE, PlatformConfig, QQ_OFFICIAL_PLATFORM_TYPE,
    QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE, SATORI_PLATFORM_TYPE, SLACK_PLATFORM_TYPE,
    TELEGRAM_PLATFORM_TYPE, WEBCHAT_PLATFORM_TYPE, WECOM_AI_BOT_PLATFORM_TYPE,
    WECOM_KF_PLATFORM_TYPE, WECOM_PLATFORM_TYPE, WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE,
};
pub use recording::{RecordingSink, SentMessage, StreamedMessage};

pub(crate) use validation::validate_platform_id;
