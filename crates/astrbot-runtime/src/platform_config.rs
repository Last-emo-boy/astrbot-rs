use astrbot_platform::{
    CONSOLE_PLATFORM_TYPE, MOCK_PLATFORM_TYPE, ONEBOT_PLATFORM_TYPE, PlatformConfig,
    WEBCHAT_PLATFORM_TYPE,
};
use serde::{Deserialize, Serialize};

use crate::defaults::{
    default_false, default_true, default_webchat_platform_id, default_webchat_server_host,
    default_webchat_server_port,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePlatformConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub platform_type: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub name: Option<String>,
}

impl RuntimePlatformConfig {
    pub fn mock(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: MOCK_PLATFORM_TYPE.to_string(),
            enabled: true,
            name: None,
        }
    }

    pub fn console(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: CONSOLE_PLATFORM_TYPE.to_string(),
            enabled: true,
            name: None,
        }
    }

    pub fn webchat(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: WEBCHAT_PLATFORM_TYPE.to_string(),
            enabled: true,
            name: None,
        }
    }

    pub fn onebot(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            platform_type: ONEBOT_PLATFORM_TYPE.to_string(),
            enabled: true,
            name: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
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
