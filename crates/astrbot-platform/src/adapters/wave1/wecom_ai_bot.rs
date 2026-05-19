use astrbot_core::{AstrbotError, Result};

use crate::{
    BuiltPlatform, PlatformBuildContext, PlatformConfig, WECOM_AI_BOT_PLATFORM_TYPE,
    adapters::wave1::common::{
        Wave1PlatformSpec, Wave1SignatureSpec, build_wave1_platform, required_secret_or_option,
        required_u16,
    },
};

pub(crate) fn build(config: &PlatformConfig, ctx: &PlatformBuildContext) -> Result<BuiltPlatform> {
    required_u16(config, "port")?;
    match config
        .option_str("wecom_ai_bot_connection_mode")
        .unwrap_or("long_connection")
    {
        "long_connection" => {
            required_secret_or_option(config, "wecomaibot_ws_bot_id")?;
            required_secret_or_option(config, "wecomaibot_ws_secret")?;
        }
        "webhook" => {
            required_secret_or_option(config, "wecomaibot_token")?;
            required_secret_or_option(config, "wecomaibot_encoding_aes_key")?;
        }
        mode => {
            return Err(AstrbotError::Platform(format!(
                "platform {} wecom_ai_bot_connection_mode must be long_connection or webhook, got {mode}",
                config.id
            )));
        }
    }
    build_wave1_platform(config, ctx, spec())
}

pub(crate) fn spec() -> Wave1PlatformSpec {
    Wave1PlatformSpec {
        platform_type: WECOM_AI_BOT_PLATFORM_TYPE,
        default_name: "WeCom AI Bot Platform",
        streaming_supported: false,
        connection_mode_option: Some("wecom_ai_bot_connection_mode"),
        default_connection_mode: "long_connection",
        api_base_url_option: "wecom_ai_bot_api_base_url",
        default_api_base_url: "https://qyapi.weixin.qq.com",
        webhook_host_option: "host",
        webhook_port_option: "port",
        webhook_path_option: "wecom_ai_bot_webhook_path",
        default_webhook_path: "/astrbot-wecom-ai-bot-webhook/callback",
        socket_url_option: "wecom_ai_bot_socket_url",
        signature: Wave1SignatureSpec::Sha1SortedFields {
            secret_key: "wecomaibot_token",
        },
    }
}
