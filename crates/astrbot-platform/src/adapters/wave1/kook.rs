use astrbot_core::{AstrbotError, Result};

use crate::{
    BuiltPlatform, KOOK_PLATFORM_TYPE, PlatformBuildContext, PlatformConfig,
    adapters::wave1::common::{
        Wave1PlatformSpec, Wave1SignatureSpec, build_wave1_platform, required_secret_or_option,
    },
};

pub(crate) fn build(config: &PlatformConfig, ctx: &PlatformBuildContext) -> Result<BuiltPlatform> {
    required_secret_or_option(config, "kook_bot_token")?;
    match config
        .option_str("kook_connection_mode")
        .unwrap_or("socket")
    {
        "socket" | "webhook" => {}
        mode => {
            return Err(AstrbotError::Platform(format!(
                "platform {} kook_connection_mode must be socket or webhook, got {mode}",
                config.id
            )));
        }
    }
    build_wave1_platform(config, ctx, spec())
}

pub(crate) fn spec() -> Wave1PlatformSpec {
    Wave1PlatformSpec {
        platform_type: KOOK_PLATFORM_TYPE,
        default_name: "KOOK Platform",
        streaming_supported: false,
        connection_mode_option: Some("kook_connection_mode"),
        default_connection_mode: "socket",
        api_base_url_option: "kook_api_base_url",
        default_api_base_url: "https://www.kookapp.cn/api/v3",
        webhook_host_option: "kook_webhook_host",
        webhook_port_option: "kook_webhook_port",
        webhook_path_option: "kook_webhook_path",
        default_webhook_path: "/astrbot-kook-webhook/callback",
        socket_url_option: "kook_socket_url",
        signature: Wave1SignatureSpec::None,
    }
}
