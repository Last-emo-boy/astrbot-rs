use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::defaults::{default_provider_timeout_secs, default_true};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeProviderSourceConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    #[serde(default = "default_true", alias = "enable")]
    pub enabled: bool,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default, alias = "key")]
    pub api_key: Option<String>,
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default = "default_provider_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub custom_extra_body: Value,
}

impl RuntimeProviderSourceConfig {
    pub fn openai(id: impl Into<String>, api_base: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::OPENAI_CHAT_PROVIDER_TYPE.to_string(),
            enabled: true,
            provider: Some("openai".to_string()),
            api_base: Some(api_base.into()),
            api_key: None,
            proxy: None,
            timeout_secs: 120,
            custom_extra_body: Value::Null,
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }
}
