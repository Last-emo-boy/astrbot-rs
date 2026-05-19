use astrbot_core::Result;

use crate::{
    BuiltPlatform, LINE_PLATFORM_TYPE, PlatformBuildContext, PlatformConfig,
    adapters::wave1::common::{
        Wave1PlatformSpec, Wave1SignatureSpec, build_wave1_platform, required_secret_or_option,
    },
};

pub(crate) fn build(config: &PlatformConfig, ctx: &PlatformBuildContext) -> Result<BuiltPlatform> {
    required_secret_or_option(config, "channel_access_token")?;
    required_secret_or_option(config, "channel_secret")?;
    build_wave1_platform(config, ctx, spec())
}

pub(crate) fn spec() -> Wave1PlatformSpec {
    Wave1PlatformSpec {
        platform_type: LINE_PLATFORM_TYPE,
        default_name: "LINE Platform",
        streaming_supported: false,
        connection_mode_option: Some("line_connection_mode"),
        default_connection_mode: "webhook",
        api_base_url_option: "line_api_base_url",
        default_api_base_url: "https://api.line.me",
        webhook_host_option: "line_webhook_host",
        webhook_port_option: "line_webhook_port",
        webhook_path_option: "line_webhook_path",
        default_webhook_path: "/astrbot-line-webhook/callback",
        socket_url_option: "line_socket_url",
        signature: Wave1SignatureSpec::LineHmacSha256 {
            secret_key: "channel_secret",
        },
    }
}
