use astrbot_core::{AstrbotError, Result};

use crate::{
    BuiltPlatform, DISCORD_PLATFORM_TYPE, PlatformBuildContext, PlatformConfig,
    adapters::wave1::common::{
        Wave1PlatformSpec, Wave1SignatureSpec, build_wave1_platform, required_secret_or_option,
    },
};

pub(crate) fn build(config: &PlatformConfig, ctx: &PlatformBuildContext) -> Result<BuiltPlatform> {
    required_secret_or_option(config, "discord_token")?;
    match config
        .option_str("discord_connection_mode")
        .unwrap_or("socket")
    {
        "socket" | "webhook" => {}
        mode => {
            return Err(AstrbotError::Platform(format!(
                "platform {} discord_connection_mode must be socket or webhook, got {mode}",
                config.id
            )));
        }
    }
    build_wave1_platform(config, ctx, spec())
}

pub(crate) fn spec() -> Wave1PlatformSpec {
    Wave1PlatformSpec {
        platform_type: DISCORD_PLATFORM_TYPE,
        default_name: "Discord Platform",
        streaming_supported: false,
        connection_mode_option: Some("discord_connection_mode"),
        default_connection_mode: "socket",
        api_base_url_option: "discord_api_base_url",
        default_api_base_url: "https://discord.com/api/v10",
        webhook_host_option: "discord_webhook_host",
        webhook_port_option: "discord_webhook_port",
        webhook_path_option: "discord_webhook_path",
        default_webhook_path: "/astrbot-discord-webhook/callback",
        socket_url_option: "discord_socket_url",
        signature: Wave1SignatureSpec::None,
    }
}
