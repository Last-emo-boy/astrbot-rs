use astrbot_core::{AstrbotError, Result};

use crate::{
    BuiltPlatform, PlatformBuildContext, PlatformConfig, SLACK_PLATFORM_TYPE,
    adapters::wave1::common::{
        Wave1PlatformSpec, Wave1SignatureSpec, build_wave1_platform, required_secret_or_option,
        required_u16,
    },
};

pub(crate) fn build(config: &PlatformConfig, ctx: &PlatformBuildContext) -> Result<BuiltPlatform> {
    required_secret_or_option(config, "bot_token")?;
    match config
        .option_str("slack_connection_mode")
        .unwrap_or("socket")
    {
        "socket" => required_secret_or_option(config, "app_token")?,
        "webhook" => {
            required_secret_or_option(config, "signing_secret")?;
            required_u16(config, "slack_webhook_port")?;
        }
        mode => {
            return Err(AstrbotError::Platform(format!(
                "platform {} slack_connection_mode must be socket or webhook, got {mode}",
                config.id
            )));
        }
    }
    build_wave1_platform(config, ctx, spec())
}

pub(crate) fn spec() -> Wave1PlatformSpec {
    Wave1PlatformSpec {
        platform_type: SLACK_PLATFORM_TYPE,
        default_name: "Slack Platform",
        streaming_supported: false,
        connection_mode_option: Some("slack_connection_mode"),
        default_connection_mode: "socket",
        api_base_url_option: "slack_api_base_url",
        default_api_base_url: "https://slack.com/api",
        webhook_host_option: "slack_webhook_host",
        webhook_port_option: "slack_webhook_port",
        webhook_path_option: "slack_webhook_path",
        default_webhook_path: "/astrbot-slack-webhook/callback",
        socket_url_option: "slack_socket_url",
        signature: Wave1SignatureSpec::SlackHmacSha256 {
            secret_key: "signing_secret",
        },
    }
}
