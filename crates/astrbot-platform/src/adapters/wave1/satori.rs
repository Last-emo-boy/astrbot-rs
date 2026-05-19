use astrbot_core::{AstrbotError, Result};

use crate::{
    BuiltPlatform, PlatformBuildContext, PlatformConfig, SATORI_PLATFORM_TYPE,
    adapters::wave1::common::{
        Wave1PlatformSpec, Wave1SignatureSpec, build_wave1_platform, required_option_str,
    },
};

pub(crate) fn build(config: &PlatformConfig, ctx: &PlatformBuildContext) -> Result<BuiltPlatform> {
    if config.option_str("satori_endpoint").is_none()
        && config.option_str("satori_socket_url").is_none()
    {
        required_option_str(config, "satori_endpoint")?;
    }
    match config
        .option_str("satori_connection_mode")
        .unwrap_or("socket")
    {
        "socket" | "webhook" => {}
        mode => {
            return Err(AstrbotError::Platform(format!(
                "platform {} satori_connection_mode must be socket or webhook, got {mode}",
                config.id
            )));
        }
    }
    build_wave1_platform(config, ctx, spec())
}

pub(crate) fn spec() -> Wave1PlatformSpec {
    Wave1PlatformSpec {
        platform_type: SATORI_PLATFORM_TYPE,
        default_name: "Satori Platform",
        streaming_supported: false,
        connection_mode_option: Some("satori_connection_mode"),
        default_connection_mode: "socket",
        api_base_url_option: "satori_api_base_url",
        default_api_base_url: "http://localhost:5140/satori/v1",
        webhook_host_option: "satori_webhook_host",
        webhook_port_option: "satori_webhook_port",
        webhook_path_option: "satori_webhook_path",
        default_webhook_path: "/astrbot-satori-webhook/callback",
        socket_url_option: "satori_endpoint",
        signature: Wave1SignatureSpec::None,
    }
}
