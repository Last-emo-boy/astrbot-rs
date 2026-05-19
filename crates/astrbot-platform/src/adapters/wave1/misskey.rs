use astrbot_core::{AstrbotError, Result};

use crate::{
    BuiltPlatform, MISSKEY_PLATFORM_TYPE, PlatformBuildContext, PlatformConfig,
    adapters::wave1::common::{
        Wave1PlatformSpec, Wave1SignatureSpec, build_wave1_platform, required_option_str,
        required_secret_or_option,
    },
};

pub(crate) fn build(config: &PlatformConfig, ctx: &PlatformBuildContext) -> Result<BuiltPlatform> {
    required_option_str(config, "misskey_instance_url")?;
    required_secret_or_option(config, "misskey_token")?;
    match config
        .option_str("misskey_connection_mode")
        .unwrap_or("socket")
    {
        "socket" | "webhook" => {}
        mode => {
            return Err(AstrbotError::Platform(format!(
                "platform {} misskey_connection_mode must be socket or webhook, got {mode}",
                config.id
            )));
        }
    }
    build_wave1_platform(config, ctx, spec())
}

pub(crate) fn spec() -> Wave1PlatformSpec {
    Wave1PlatformSpec {
        platform_type: MISSKEY_PLATFORM_TYPE,
        default_name: "Misskey Platform",
        streaming_supported: false,
        connection_mode_option: Some("misskey_connection_mode"),
        default_connection_mode: "socket",
        api_base_url_option: "misskey_api_base_url",
        default_api_base_url: "https://misskey.example/api",
        webhook_host_option: "misskey_webhook_host",
        webhook_port_option: "misskey_webhook_port",
        webhook_path_option: "misskey_webhook_path",
        default_webhook_path: "/astrbot-misskey-webhook/callback",
        socket_url_option: "misskey_socket_url",
        signature: Wave1SignatureSpec::None,
    }
}
