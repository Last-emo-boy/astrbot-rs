use std::time::Duration;

use astrbot_provider::ChatProviderConfig;
use serde::{Deserialize, Serialize};

use crate::defaults::{DEFAULT_MOCK_RESPONSE, default_provider_timeout_secs, default_true};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeChatProviderConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_provider_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub mock_response: Option<String>,
}

impl RuntimeChatProviderConfig {
    pub fn mock(id: impl Into<String>, response: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::MOCK_CHAT_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout_secs: 120,
            mock_response: Some(response.into()),
        }
    }

    pub fn openai_compatible(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::OPENAI_CHAT_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 120,
            mock_response: None,
        }
    }

    pub fn openai_compatible_with_type(
        provider_type: impl Into<String>,
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let mut config = Self::openai_compatible(id, api_base, model);
        config.provider_type = provider_type.into();
        config
    }

    pub fn anthropic(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::ANTHROPIC_CHAT_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 120,
            mock_response: None,
        }
    }

    pub fn google_genai(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::GOOGLE_GENAI_CHAT_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 120,
            mock_response: None,
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_timeout_secs(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }
}

impl From<RuntimeChatProviderConfig> for ChatProviderConfig {
    fn from(config: RuntimeChatProviderConfig) -> Self {
        let mut provider_config = match config.provider_type.as_str() {
            astrbot_provider::MOCK_CHAT_PROVIDER_TYPE => ChatProviderConfig::mock(
                config.id,
                config
                    .mock_response
                    .unwrap_or_else(|| DEFAULT_MOCK_RESPONSE.to_string()),
            ),
            astrbot_provider::OPENAI_CHAT_PROVIDER_TYPE => ChatProviderConfig::openai_compatible(
                config.id,
                config.api_base.unwrap_or_default(),
                config.model.unwrap_or_default(),
            ),
            astrbot_provider::ANTHROPIC_CHAT_PROVIDER_TYPE => ChatProviderConfig::anthropic(
                config.id,
                config
                    .api_base
                    .unwrap_or_else(|| "https://api.anthropic.com".to_string()),
                config.model.unwrap_or_default(),
            ),
            astrbot_provider::GOOGLE_GENAI_CHAT_PROVIDER_TYPE => ChatProviderConfig::google_genai(
                config.id,
                config
                    .api_base
                    .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string()),
                config.model.unwrap_or_default(),
            ),
            _ => ChatProviderConfig {
                id: config.id,
                provider_type: config.provider_type,
                enabled: true,
                model: config.model,
                api_base: config.api_base,
                api_key: None,
                timeout: Duration::from_secs(config.timeout_secs),
                custom_headers: Default::default(),
                mock_response: config.mock_response,
            },
        };

        provider_config.enabled = config.enabled;
        provider_config.timeout = Duration::from_secs(config.timeout_secs);
        provider_config.api_key = config.api_key;
        provider_config
    }
}
