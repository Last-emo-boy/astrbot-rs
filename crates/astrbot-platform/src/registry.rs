use std::collections::HashMap;
use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};

use crate::{
    AIOCQHTTP_PLATFORM_TYPE, BuiltPlatform, CONSOLE_PLATFORM_TYPE, ConsolePlatform, ConsoleSink,
    DINGTALK_PLATFORM_TYPE, DISCORD_PLATFORM_TYPE, KOOK_PLATFORM_TYPE, LARK_PLATFORM_TYPE,
    LINE_PLATFORM_TYPE, MISSKEY_PLATFORM_TYPE, MOCK_PLATFORM_TYPE, MockPlatform,
    ONEBOT_PLATFORM_TYPE, OneBotPlatform, OneBotTransport, PlatformBuildContext, PlatformConfig,
    QQ_OFFICIAL_PLATFORM_TYPE, QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE, RecordingSink,
    SATORI_PLATFORM_TYPE, SLACK_PLATFORM_TYPE, TELEGRAM_PLATFORM_TYPE, WEBCHAT_PLATFORM_TYPE,
    WECOM_AI_BOT_PLATFORM_TYPE, WECOM_KF_PLATFORM_TYPE, WECOM_PLATFORM_TYPE,
    WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE, WebChatPlatform, adapters::wave1,
};
type PlatformFactory =
    Arc<dyn Fn(&PlatformConfig, &PlatformBuildContext) -> Result<BuiltPlatform> + Send + Sync>;

#[derive(Clone, Default)]
pub struct PlatformRegistry {
    factories: HashMap<String, PlatformFactory>,
}

impl PlatformRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtin_platforms() -> Self {
        let mut registry = Self::new();
        registry
            .register_platform(MOCK_PLATFORM_TYPE, |config, ctx| {
                let name = config
                    .name
                    .clone()
                    .unwrap_or_else(|| "Mock Platform".to_string());
                let sink = Arc::new(RecordingSink::default());
                let platform = Arc::new(MockPlatform::with_identity(
                    config.id.clone(),
                    name,
                    ctx.event_sender(),
                    sink,
                ));
                Ok(BuiltPlatform::mock(platform))
            })
            .expect("built-in mock platform type should register once");
        registry
            .register_platform(CONSOLE_PLATFORM_TYPE, |config, ctx| {
                let name = config
                    .name
                    .clone()
                    .unwrap_or_else(|| "Console Platform".to_string());
                let sink = Arc::new(ConsoleSink::default());
                let platform = Arc::new(ConsolePlatform::with_identity(
                    config.id.clone(),
                    name,
                    ctx.event_sender(),
                    sink.clone(),
                ));
                Ok(BuiltPlatform::with_recording_sink(platform, sink))
            })
            .expect("built-in console platform type should register once");
        registry
            .register_platform(WEBCHAT_PLATFORM_TYPE, |config, ctx| {
                let name = config
                    .name
                    .clone()
                    .unwrap_or_else(|| "WebChat Platform".to_string());
                let sink = Arc::new(RecordingSink::default());
                let platform = Arc::new(WebChatPlatform::with_identity(
                    config.id.clone(),
                    name,
                    ctx.event_sender(),
                    sink,
                ));
                Ok(BuiltPlatform::webchat(platform))
            })
            .expect("built-in webchat platform type should register once");
        registry
            .register_platform(ONEBOT_PLATFORM_TYPE, build_onebot_platform)
            .expect("built-in onebot platform type should register once");
        registry
            .register_platform(AIOCQHTTP_PLATFORM_TYPE, build_onebot_platform)
            .expect("built-in aiocqhttp platform alias should register once");
        registry
            .register_platform(TELEGRAM_PLATFORM_TYPE, wave1::telegram::build)
            .expect("built-in telegram platform type should register once");
        registry
            .register_platform(SLACK_PLATFORM_TYPE, wave1::slack::build)
            .expect("built-in slack platform type should register once");
        registry
            .register_platform(LARK_PLATFORM_TYPE, wave1::lark::build)
            .expect("built-in lark platform type should register once");
        registry
            .register_platform(LINE_PLATFORM_TYPE, wave1::line::build)
            .expect("built-in line platform type should register once");
        registry
            .register_platform(WECOM_PLATFORM_TYPE, wave1::wecom::build)
            .expect("built-in wecom platform type should register once");
        registry
            .register_platform(WECOM_AI_BOT_PLATFORM_TYPE, wave1::wecom_ai_bot::build)
            .expect("built-in wecom_ai_bot platform type should register once");
        registry
            .register_platform(DINGTALK_PLATFORM_TYPE, wave1::dingtalk::build)
            .expect("built-in dingtalk platform type should register once");
        registry
            .register_platform(DISCORD_PLATFORM_TYPE, wave1::discord::build)
            .expect("built-in discord platform type should register once");
        registry
            .register_platform(KOOK_PLATFORM_TYPE, wave1::kook::build)
            .expect("built-in kook platform type should register once");
        registry
            .register_platform(MISSKEY_PLATFORM_TYPE, wave1::misskey::build)
            .expect("built-in misskey platform type should register once");
        registry
            .register_platform(SATORI_PLATFORM_TYPE, wave1::satori::build)
            .expect("built-in satori platform type should register once");
        registry
            .register_platform(QQ_OFFICIAL_PLATFORM_TYPE, wave1::qq_official::build)
            .expect("built-in qq_official platform type should register once");
        registry
            .register_platform(
                QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE,
                wave1::qq_official_webhook::build,
            )
            .expect("built-in qq_official_webhook platform type should register once");
        registry
            .register_platform(WECOM_KF_PLATFORM_TYPE, wave1::wecom_kf::build)
            .expect("built-in wecom_kf platform type should register once");
        registry
            .register_platform(
                WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE,
                wave1::weixin_official_account::build,
            )
            .expect("built-in weixin_official_account platform type should register once");
        registry
    }

    pub fn register_platform(
        &mut self,
        platform_type: impl Into<String>,
        factory: impl Fn(&PlatformConfig, &PlatformBuildContext) -> Result<BuiltPlatform>
        + Send
        + Sync
        + 'static,
    ) -> Result<()> {
        let platform_type = platform_type.into();
        if self.factories.contains_key(&platform_type) {
            return Err(AstrbotError::Platform(format!(
                "platform type {platform_type} is already registered"
            )));
        }
        self.factories.insert(platform_type, Arc::new(factory));
        Ok(())
    }

    pub fn has_platform(&self, platform_type: &str) -> bool {
        self.factories.contains_key(platform_type)
    }

    pub fn build_platform(
        &self,
        config: &PlatformConfig,
        ctx: &PlatformBuildContext,
    ) -> Result<BuiltPlatform> {
        let factory = self.factories.get(&config.platform_type).ok_or_else(|| {
            AstrbotError::Platform(format!(
                "platform type {} is not registered",
                config.platform_type
            ))
        })?;
        factory(config, ctx)
    }
}

fn build_onebot_platform(
    config: &PlatformConfig,
    ctx: &PlatformBuildContext,
) -> Result<BuiltPlatform> {
    validate_required_option_str(config, "ws_reverse_host")?;
    validate_required_u16(config, "ws_reverse_port")?;
    let host = config
        .option_str("ws_reverse_host")
        .expect("validated onebot host should exist")
        .to_string();
    let port = config
        .option_u16("ws_reverse_port")
        .expect("validated onebot port should exist");
    let token = config
        .secret_or_option_str("ws_reverse_token")
        .map(str::to_string);
    let name = config
        .name
        .clone()
        .unwrap_or_else(|| "OneBot Platform".to_string());
    let sink = Arc::new(RecordingSink::default());
    let platform = Arc::new(
        OneBotPlatform::with_identity(config.id.clone(), name, ctx.event_sender(), sink)
            .with_transport(OneBotTransport::reverse_websocket_with_token(
                host, port, token,
            )),
    );
    Ok(BuiltPlatform::onebot(platform))
}

fn validate_required_option_str(config: &PlatformConfig, key: &str) -> Result<()> {
    let value = config.option_str(key).unwrap_or_default().trim();
    if value.is_empty() {
        return Err(AstrbotError::Platform(format!(
            "platform {} ({}) requires option {key}",
            config.id, config.platform_type
        )));
    }
    Ok(())
}

fn validate_required_u16(config: &PlatformConfig, key: &str) -> Result<()> {
    if config.option_u16(key).is_none() {
        return Err(AstrbotError::Platform(format!(
            "platform {} ({}) requires numeric option {key}",
            config.id, config.platform_type
        )));
    }
    Ok(())
}
