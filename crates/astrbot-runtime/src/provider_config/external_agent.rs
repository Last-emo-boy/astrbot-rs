use std::collections::BTreeMap;

use astrbot_agent::{ExternalAgentConnectorConfig, ExternalAgentConnectorKind};
use serde::{Deserialize, Serialize};

use crate::defaults::{default_provider_timeout_secs, default_true};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeExternalAgentConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub runner_type: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub app_id: Option<String>,
    #[serde(default)]
    pub bot_id: Option<String>,
    #[serde(default = "default_provider_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
}

impl RuntimeExternalAgentConfig {
    pub fn new(id: impl Into<String>, runner_type: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            runner_type: runner_type.into(),
            enabled: true,
            api_base: None,
            api_key: None,
            app_id: None,
            bot_id: None,
            timeout_secs: 120,
            stream: false,
            options: BTreeMap::new(),
        }
    }

    pub fn coze(
        id: impl Into<String>,
        api_base: impl Into<String>,
        bot_id: impl Into<String>,
    ) -> Self {
        Self::new(id, "coze")
            .with_api_base(api_base)
            .with_bot_id(bot_id)
    }

    pub fn dify(
        id: impl Into<String>,
        api_base: impl Into<String>,
        app_id: impl Into<String>,
    ) -> Self {
        Self::new(id, "dify")
            .with_api_base(api_base)
            .with_app_id(app_id)
    }

    pub fn dashscope(id: impl Into<String>, app_id: impl Into<String>) -> Self {
        Self::new(id, "dashscope").with_app_id(app_id)
    }

    pub fn deerflow(id: impl Into<String>, api_base: impl Into<String>) -> Self {
        Self::new(id, "deerflow").with_api_base(api_base)
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = non_empty_option(api_base);
        self
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = non_empty_option(api_key);
        self
    }

    pub fn with_app_id(mut self, app_id: impl Into<String>) -> Self {
        self.app_id = non_empty_option(app_id);
        self
    }

    pub fn with_bot_id(mut self, bot_id: impl Into<String>) -> Self {
        self.bot_id = non_empty_option(bot_id);
        self
    }

    pub fn with_timeout_secs(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    pub fn with_streaming(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        if !key.trim().is_empty() && !value.trim().is_empty() {
            self.options
                .insert(key.trim().to_string(), value.trim().to_string());
        }
        self
    }
}

impl From<RuntimeExternalAgentConfig> for ExternalAgentConnectorConfig {
    fn from(config: RuntimeExternalAgentConfig) -> Self {
        let mut connector = ExternalAgentConnectorConfig::new(
            config.id,
            ExternalAgentConnectorKind::from(config.runner_type.as_str()),
        )
        .with_streaming(config.stream);
        connector.api_base = config.api_base;
        connector.api_key = config.api_key;
        connector.app_id = config.app_id;
        connector.bot_id = config.bot_id;
        connector.timeout_secs = config.timeout_secs;
        connector.options = config.options;
        connector
    }
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then(|| value.trim().to_string())
}
