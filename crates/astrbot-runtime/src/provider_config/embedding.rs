use std::time::Duration;

use astrbot_provider::EmbeddingProviderConfig;
use serde::{Deserialize, Serialize};

use crate::defaults::{default_provider_timeout_secs, default_true};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEmbeddingProviderConfig {
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
    pub dimensions: Option<usize>,
}

impl RuntimeEmbeddingProviderConfig {
    pub fn mock(id: impl Into<String>, dimensions: usize) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::MOCK_EMBEDDING_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout_secs: 120,
            dimensions: Some(dimensions.max(1)),
        }
    }

    pub fn openai(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::OPENAI_EMBEDDING_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 120,
            dimensions: Some(1024),
        }
    }

    pub fn gemini(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::GEMINI_EMBEDDING_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 120,
            dimensions: Some(768),
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

    pub fn with_dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = Some(dimensions.max(1));
        self
    }
}

impl From<RuntimeEmbeddingProviderConfig> for EmbeddingProviderConfig {
    fn from(config: RuntimeEmbeddingProviderConfig) -> Self {
        let configured_dimensions = config.dimensions.map(|dimensions| dimensions.max(1));
        let mock_dimensions = configured_dimensions.unwrap_or(3);
        let mut provider_config = match config.provider_type.as_str() {
            astrbot_provider::MOCK_EMBEDDING_PROVIDER_TYPE => {
                EmbeddingProviderConfig::mock(config.id, vec![0.0_f32; mock_dimensions])
            }
            astrbot_provider::OPENAI_EMBEDDING_PROVIDER_TYPE => EmbeddingProviderConfig::openai(
                config.id,
                config.api_base.unwrap_or_default(),
                config
                    .model
                    .unwrap_or_else(|| "text-embedding-3-small".to_string()),
            ),
            astrbot_provider::GEMINI_EMBEDDING_PROVIDER_TYPE => EmbeddingProviderConfig::gemini(
                config.id,
                config
                    .api_base
                    .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string()),
                config
                    .model
                    .unwrap_or_else(|| "gemini-embedding-001".to_string()),
            ),
            _ => EmbeddingProviderConfig {
                id: config.id,
                provider_type: config.provider_type,
                enabled: true,
                model: config.model,
                api_base: config.api_base,
                api_key: None,
                timeout: Duration::from_secs(config.timeout_secs),
                custom_headers: Default::default(),
                dimensions: configured_dimensions,
                mock_embedding: None,
            },
        };

        provider_config.enabled = config.enabled;
        provider_config.timeout = Duration::from_secs(config.timeout_secs);
        provider_config.api_key = config.api_key;
        if let Some(dimensions) = configured_dimensions {
            provider_config.dimensions = Some(dimensions);
        }
        provider_config
    }
}
