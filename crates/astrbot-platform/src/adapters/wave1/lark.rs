use astrbot_core::{AstrbotError, Result};

use crate::{
    BuiltPlatform, LARK_PLATFORM_TYPE, PlatformBuildContext, PlatformConfig,
    adapters::wave1::common::{
        Wave1PlatformSpec, Wave1SignatureSpec, build_wave1_platform, required_option_str,
        required_secret_or_option,
    },
};

pub(crate) fn build(config: &PlatformConfig, ctx: &PlatformBuildContext) -> Result<BuiltPlatform> {
    required_option_str(config, "app_id")?;
    required_secret_or_option(config, "app_secret")?;
    match config
        .option_str("lark_connection_mode")
        .unwrap_or("socket")
    {
        "socket" | "webhook" => {}
        mode => {
            return Err(AstrbotError::Platform(format!(
                "platform {} lark_connection_mode must be socket or webhook, got {mode}",
                config.id
            )));
        }
    }
    build_wave1_platform(config, ctx, spec())
}

pub(crate) fn spec() -> Wave1PlatformSpec {
    Wave1PlatformSpec {
        platform_type: LARK_PLATFORM_TYPE,
        default_name: "Lark Platform",
        streaming_supported: true,
        connection_mode_option: Some("lark_connection_mode"),
        default_connection_mode: "socket",
        api_base_url_option: "lark_api_base_url",
        default_api_base_url: "https://open.feishu.cn/open-apis",
        webhook_host_option: "lark_webhook_host",
        webhook_port_option: "lark_webhook_port",
        webhook_path_option: "lark_webhook_path",
        default_webhook_path: "/astrbot-lark-webhook/callback",
        socket_url_option: "lark_socket_url",
        signature: Wave1SignatureSpec::None,
    }
}
