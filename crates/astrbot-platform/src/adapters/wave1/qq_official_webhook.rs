use astrbot_core::Result;

use crate::{
    BuiltPlatform, PlatformBuildContext, PlatformConfig, QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE,
    adapters::wave1::common::{
        Wave1PlatformSpec, Wave1SignatureSpec, build_wave1_platform, required_option_str,
        required_secret_or_option,
    },
};

pub(crate) fn build(config: &PlatformConfig, ctx: &PlatformBuildContext) -> Result<BuiltPlatform> {
    required_option_str(config, "appid")?;
    required_secret_or_option(config, "secret")?;
    build_wave1_platform(config, ctx, spec())
}

pub(crate) fn spec() -> Wave1PlatformSpec {
    Wave1PlatformSpec {
        platform_type: QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE,
        default_name: "QQ Official Webhook Platform",
        streaming_supported: false,
        connection_mode_option: Some("qq_official_webhook_connection_mode"),
        default_connection_mode: "webhook",
        api_base_url_option: "qq_official_webhook_api_base_url",
        default_api_base_url: "https://api.sgroup.qq.com",
        webhook_host_option: "qq_official_webhook_host",
        webhook_port_option: "qq_official_webhook_port",
        webhook_path_option: "qq_official_webhook_path",
        default_webhook_path: "/astrbot-qq-official-webhook/callback",
        socket_url_option: "qq_official_webhook_socket_url",
        signature: Wave1SignatureSpec::None,
    }
}
