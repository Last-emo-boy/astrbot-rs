use astrbot_core::Result;

use crate::{
    BuiltPlatform, PlatformBuildContext, PlatformConfig, WECOM_PLATFORM_TYPE,
    adapters::wave1::common::{
        Wave1PlatformSpec, Wave1SignatureSpec, build_wave1_platform, required_secret_or_option,
    },
};

pub(crate) fn build(config: &PlatformConfig, ctx: &PlatformBuildContext) -> Result<BuiltPlatform> {
    required_secret_or_option(config, "corpid")?;
    required_secret_or_option(config, "secret")?;
    build_wave1_platform(config, ctx, spec())
}

pub(crate) fn spec() -> Wave1PlatformSpec {
    Wave1PlatformSpec {
        platform_type: WECOM_PLATFORM_TYPE,
        default_name: "WeCom Platform",
        streaming_supported: false,
        connection_mode_option: Some("wecom_connection_mode"),
        default_connection_mode: "webhook",
        api_base_url_option: "wecom_api_base_url",
        default_api_base_url: "https://qyapi.weixin.qq.com",
        webhook_host_option: "wecom_webhook_host",
        webhook_port_option: "wecom_webhook_port",
        webhook_path_option: "wecom_webhook_path",
        default_webhook_path: "/astrbot-wecom-webhook/callback",
        socket_url_option: "wecom_socket_url",
        signature: Wave1SignatureSpec::Sha1SortedFields {
            secret_key: "token",
        },
    }
}
