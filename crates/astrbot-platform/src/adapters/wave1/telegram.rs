use astrbot_core::Result;

use crate::{
    BuiltPlatform, PlatformBuildContext, PlatformConfig, TELEGRAM_PLATFORM_TYPE,
    adapters::wave1::common::{
        Wave1PlatformSpec, Wave1SignatureSpec, build_wave1_platform, required_secret_or_option,
    },
};

pub(crate) fn build(config: &PlatformConfig, ctx: &PlatformBuildContext) -> Result<BuiltPlatform> {
    required_secret_or_option(config, "telegram_token")?;
    build_wave1_platform(config, ctx, spec())
}

pub(crate) fn spec() -> Wave1PlatformSpec {
    Wave1PlatformSpec {
        platform_type: TELEGRAM_PLATFORM_TYPE,
        default_name: "Telegram Platform",
        streaming_supported: false,
        connection_mode_option: Some("telegram_connection_mode"),
        default_connection_mode: "webhook",
        api_base_url_option: "telegram_api_base_url",
        default_api_base_url: "https://api.telegram.org/bot",
        webhook_host_option: "telegram_webhook_host",
        webhook_port_option: "telegram_webhook_port",
        webhook_path_option: "telegram_webhook_path",
        default_webhook_path: "/astrbot-telegram-webhook/callback",
        socket_url_option: "telegram_socket_url",
        signature: Wave1SignatureSpec::None,
    }
}
