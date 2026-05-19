use std::collections::BTreeMap;
use std::fmt;

use astrbot_platform::{
    AIOCQHTTP_PLATFORM_TYPE, CONSOLE_PLATFORM_TYPE, DINGTALK_PLATFORM_TYPE, LARK_PLATFORM_TYPE,
    LINE_PLATFORM_TYPE, MOCK_PLATFORM_TYPE, ONEBOT_PLATFORM_TYPE, PlatformConfig,
    SLACK_PLATFORM_TYPE, TELEGRAM_PLATFORM_TYPE, WEBCHAT_PLATFORM_TYPE, WECOM_AI_BOT_PLATFORM_TYPE,
    WECOM_PLATFORM_TYPE,
};
use astrbot_tool::CommandPermission;
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

use crate::defaults::{
    default_false, default_true, default_webchat_platform_id, default_webchat_server_host,
    default_webchat_server_port,
};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePlatformConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub platform_type: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub secrets: BTreeMap<String, String>,
}

impl fmt::Debug for RuntimePlatformConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redacted_secrets = self
            .secrets
            .keys()
            .map(|key| (key, "<redacted>"))
            .collect::<BTreeMap<_, _>>();
        f.debug_struct("RuntimePlatformConfig")
            .field("id", &self.id)
            .field("platform_type", &self.platform_type)
            .field("enabled", &self.enabled)
            .field("name", &self.name)
            .field("options", &self.options)
            .field("secrets", &redacted_secrets)
            .finish()
    }
}

impl RuntimePlatformConfig {
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

    pub fn aiocqhttp(id: impl Into<String>) -> Self {
        Self::new(id, AIOCQHTTP_PLATFORM_TYPE)
            .with_option_string("ws_reverse_host", "0.0.0.0")
            .with_option_u16("ws_reverse_port", 6199)
    }

    pub fn telegram(id: impl Into<String>, token: impl Into<String>) -> Self {
        Self::new(id, TELEGRAM_PLATFORM_TYPE).with_secret("telegram_token", token)
    }

    pub fn slack_socket(
        id: impl Into<String>,
        bot_token: impl Into<String>,
        app_token: impl Into<String>,
    ) -> Self {
        Self::new(id, SLACK_PLATFORM_TYPE)
            .with_option_string("slack_connection_mode", "socket")
            .with_secret("bot_token", bot_token)
            .with_secret("app_token", app_token)
    }

    pub fn lark(
        id: impl Into<String>,
        app_id: impl Into<String>,
        app_secret: impl Into<String>,
    ) -> Self {
        Self::new(id, LARK_PLATFORM_TYPE)
            .with_option_string("app_id", app_id)
            .with_secret("app_secret", app_secret)
    }

    pub fn line(
        id: impl Into<String>,
        channel_access_token: impl Into<String>,
        channel_secret: impl Into<String>,
    ) -> Self {
        Self::new(id, LINE_PLATFORM_TYPE)
            .with_secret("channel_access_token", channel_access_token)
            .with_secret("channel_secret", channel_secret)
    }

    pub fn wecom(
        id: impl Into<String>,
        corpid: impl Into<String>,
        secret: impl Into<String>,
    ) -> Self {
        Self::new(id, WECOM_PLATFORM_TYPE)
            .with_secret("corpid", corpid)
            .with_secret("secret", secret)
    }

    pub fn wecom_ai_bot_long_connection(
        id: impl Into<String>,
        bot_id: impl Into<String>,
        secret: impl Into<String>,
    ) -> Self {
        Self::new(id, WECOM_AI_BOT_PLATFORM_TYPE)
            .with_option_string("wecom_ai_bot_connection_mode", "long_connection")
            .with_option_u16("port", 6198)
            .with_secret("wecomaibot_ws_bot_id", bot_id)
            .with_secret("wecomaibot_ws_secret", secret)
    }

    pub fn dingtalk(
        id: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        Self::new(id, DINGTALK_PLATFORM_TYPE)
            .with_option_string("client_id", client_id)
            .with_secret("client_secret", client_secret)
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
}

impl From<RuntimePlatformConfig> for PlatformConfig {
    fn from(config: RuntimePlatformConfig) -> Self {
        let mut platform_config = PlatformConfig {
            id: config.id,
            platform_type: config.platform_type,
            enabled: config.enabled,
            name: config.name,
            options: config.options,
            secrets: config.secrets,
        };
        if !platform_config.enabled {
            platform_config = platform_config.disabled();
        }
        platform_config
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCommandPluginConfig {
    pub plugin_name: String,
    pub handler_name: String,
    pub command: String,
    pub response: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub permission: CommandPermission,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeWebChatServerConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    #[serde(default = "default_webchat_platform_id")]
    pub platform_id: String,
    #[serde(default = "default_webchat_server_host")]
    pub host: String,
    #[serde(default = "default_webchat_server_port")]
    pub port: u16,
}

impl Default for RuntimeWebChatServerConfig {
    fn default() -> Self {
        Self {
            enabled: default_false(),
            platform_id: default_webchat_platform_id(),
            host: default_webchat_server_host(),
            port: default_webchat_server_port(),
        }
    }
}

impl RuntimeWebChatServerConfig {
    pub fn enabled(platform_id: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
        Self {
            enabled: true,
            platform_id: platform_id.into(),
            host: host.into(),
            port,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}
