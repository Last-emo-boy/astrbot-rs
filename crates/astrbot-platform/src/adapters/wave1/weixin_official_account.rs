use astrbot_core::Result;

use crate::{
    BuiltPlatform, PlatformBuildContext, PlatformConfig, WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE,
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
        platform_type: WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE,
        default_name: "Weixin Official Account Platform",
        streaming_supported: false,
        connection_mode_option: Some("weixin_official_account_connection_mode"),
        default_connection_mode: "webhook",
        api_base_url_option: "weixin_official_account_api_base_url",
        default_api_base_url: "https://api.weixin.qq.com/cgi-bin",
        webhook_host_option: "weixin_official_account_webhook_host",
        webhook_port_option: "weixin_official_account_webhook_port",
        webhook_path_option: "weixin_official_account_webhook_path",
        default_webhook_path: "/astrbot-weixin-official-account-webhook/callback",
        socket_url_option: "weixin_official_account_socket_url",
        signature: Wave1SignatureSpec::Sha1SortedFields {
            secret_key: "token",
        },
    }
}
