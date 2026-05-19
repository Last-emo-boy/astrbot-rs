use astrbot_core::Result;

use crate::{
    BuiltPlatform, DINGTALK_PLATFORM_TYPE, PlatformBuildContext, PlatformConfig,
    adapters::wave1::common::{
        Wave1PlatformSpec, Wave1SignatureSpec, build_wave1_platform, required_option_str,
        required_secret_or_option,
    },
};

pub(crate) fn build(config: &PlatformConfig, ctx: &PlatformBuildContext) -> Result<BuiltPlatform> {
    required_option_str(config, "client_id")?;
    required_secret_or_option(config, "client_secret")?;
    build_wave1_platform(config, ctx, spec())
}

pub(crate) fn spec() -> Wave1PlatformSpec {
    Wave1PlatformSpec {
        platform_type: DINGTALK_PLATFORM_TYPE,
        default_name: "DingTalk Platform",
        streaming_supported: true,
        connection_mode_option: Some("dingtalk_connection_mode"),
        default_connection_mode: "socket",
        api_base_url_option: "dingtalk_api_base_url",
        default_api_base_url: "https://api.dingtalk.com",
        webhook_host_option: "dingtalk_webhook_host",
        webhook_port_option: "dingtalk_webhook_port",
        webhook_path_option: "dingtalk_webhook_path",
        default_webhook_path: "/astrbot-dingtalk-webhook/callback",
        socket_url_option: "dingtalk_socket_url",
        signature: Wave1SignatureSpec::DingTalkHmacSha256 {
            secret_key: "client_secret",
        },
    }
}
