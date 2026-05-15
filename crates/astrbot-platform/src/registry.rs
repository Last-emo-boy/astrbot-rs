use std::collections::HashMap;
use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};

use crate::{
    BuiltPlatform, CONSOLE_PLATFORM_TYPE, ConsolePlatform, ConsoleSink, MOCK_PLATFORM_TYPE,
    MockPlatform, ONEBOT_PLATFORM_TYPE, OneBotPlatform, PlatformBuildContext, PlatformConfig,
    RecordingSink, WEBCHAT_PLATFORM_TYPE, WebChatPlatform,
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
            .register_platform(ONEBOT_PLATFORM_TYPE, |config, ctx| {
                let name = config
                    .name
                    .clone()
                    .unwrap_or_else(|| "OneBot Platform".to_string());
                let sink = Arc::new(RecordingSink::default());
                let platform = Arc::new(OneBotPlatform::with_identity(
                    config.id.clone(),
                    name,
                    ctx.event_sender(),
                    sink,
                ));
                Ok(BuiltPlatform::onebot(platform))
            })
            .expect("built-in onebot platform type should register once");
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
