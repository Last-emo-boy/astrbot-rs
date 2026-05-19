use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Number, Value};

pub const CONSOLE_PLATFORM_TYPE: &str = "console";
pub const WEBCHAT_PLATFORM_TYPE: &str = "webchat";
pub const ONEBOT_PLATFORM_TYPE: &str = "onebot";
pub const AIOCQHTTP_PLATFORM_TYPE: &str = "aiocqhttp";
pub const TELEGRAM_PLATFORM_TYPE: &str = "telegram";
pub const SLACK_PLATFORM_TYPE: &str = "slack";
pub const LARK_PLATFORM_TYPE: &str = "lark";
pub const LINE_PLATFORM_TYPE: &str = "line";
pub const WECOM_PLATFORM_TYPE: &str = "wecom";
pub const WECOM_AI_BOT_PLATFORM_TYPE: &str = "wecom_ai_bot";
pub const DINGTALK_PLATFORM_TYPE: &str = "dingtalk";
pub const DISCORD_PLATFORM_TYPE: &str = "discord";
pub const KOOK_PLATFORM_TYPE: &str = "kook";
pub const MISSKEY_PLATFORM_TYPE: &str = "misskey";
pub const SATORI_PLATFORM_TYPE: &str = "satori";
pub const QQ_OFFICIAL_PLATFORM_TYPE: &str = "qq_official";
pub const QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE: &str = "qq_official_webhook";
pub const WECOM_KF_PLATFORM_TYPE: &str = "wecom_kf";
pub const WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE: &str = "weixin_official_account";
pub const MOCK_PLATFORM_TYPE: &str = "mock";

pub struct PlatformConfig {
    pub id: String,
    pub platform_type: String,
    pub enabled: bool,
    pub name: Option<String>,
    pub options: BTreeMap<String, Value>,
    pub secrets: BTreeMap<String, String>,
}

impl fmt::Debug for PlatformConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redacted_secrets = self
            .secrets
            .keys()
            .map(|key| (key, "<redacted>"))
            .collect::<BTreeMap<_, _>>();
        f.debug_struct("PlatformConfig")
            .field("id", &self.id)
            .field("platform_type", &self.platform_type)
            .field("enabled", &self.enabled)
            .field("name", &self.name)
            .field("options", &self.options)
            .field("secrets", &redacted_secrets)
            .finish()
    }
}

impl PlatformConfig {
    pub fn new(id: impl Into<String>, platform_type: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: platform_type.into(),
            enabled: true,
            name: None,
            options: BTreeMap::new(),
            secrets: BTreeMap::new(),
        }
    }

    pub fn mock(id: impl Into<String>) -> Self {
        Self::new(id, MOCK_PLATFORM_TYPE)
    }

    pub fn console(id: impl Into<String>) -> Self {
        Self::new(id, CONSOLE_PLATFORM_TYPE)
    }

    pub fn webchat(id: impl Into<String>) -> Self {
        Self::new(id, WEBCHAT_PLATFORM_TYPE)
    }

    pub fn onebot(id: impl Into<String>) -> Self {
        Self::new(id, ONEBOT_PLATFORM_TYPE)
            .with_option_string("ws_reverse_host", "0.0.0.0")
            .with_option_u16("ws_reverse_port", 6199)
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn with_option(mut self, key: impl Into<String>, value: Value) -> Self {
        self.options.insert(key.into(), value);
        self
    }

    pub fn with_option_string(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), Value::String(value.into()));
        self
    }

    pub fn with_option_bool(mut self, key: impl Into<String>, value: bool) -> Self {
        self.options.insert(key.into(), Value::Bool(value));
        self
    }

    pub fn with_option_u16(mut self, key: impl Into<String>, value: u16) -> Self {
        self.options
            .insert(key.into(), Value::Number(Number::from(u64::from(value))));
        self
    }

    pub fn with_secret(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.secrets.insert(key.into(), value.into());
        self
    }

    pub fn option_str(&self, key: &str) -> Option<&str> {
        self.options.get(key).and_then(Value::as_str)
    }

    pub fn option_bool(&self, key: &str) -> Option<bool> {
        self.options.get(key).and_then(Value::as_bool)
    }

    pub fn option_u16(&self, key: &str) -> Option<u16> {
        self.options
            .get(key)
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
    }

    pub fn secret(&self, key: &str) -> Option<&str> {
        self.secrets.get(key).map(String::as_str)
    }

    pub fn secret_or_option_str(&self, key: &str) -> Option<&str> {
        self.secret(key).or_else(|| self.option_str(key))
    }
}
