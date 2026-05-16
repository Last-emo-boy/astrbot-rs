use std::time::Duration;

use astrbot_provider::RerankProviderConfig;
use serde::{Deserialize, Serialize};

use crate::defaults::{default_provider_timeout_secs, default_true};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRerankProviderConfig {
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
    pub mock_score_count: Option<usize>,
    #[serde(default)]
    pub launch_model_if_not_running: bool,
}

impl RuntimeRerankProviderConfig {
    pub fn mock(id: impl Into<String>, score_count: usize) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::MOCK_RERANK_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout_secs: 120,
            mock_score_count: Some(score_count.max(1)),
            launch_model_if_not_running: false,
        }
    }

    pub fn vllm(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::VLLM_RERANK_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 120,
            mock_score_count: None,
            launch_model_if_not_running: false,
        }
    }

    pub fn bailian(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::BAILIAN_RERANK_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 120,
            mock_score_count: None,
            launch_model_if_not_running: false,
        }
    }

    pub fn xinference(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::XINFERENCE_RERANK_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 180,
            mock_score_count: None,
            launch_model_if_not_running: false,
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

    pub fn with_launch_model_if_not_running(mut self, launch_model_if_not_running: bool) -> Self {
        self.launch_model_if_not_running = launch_model_if_not_running;
        self
    }
}

impl From<RuntimeRerankProviderConfig> for RerankProviderConfig {
    fn from(config: RuntimeRerankProviderConfig) -> Self {
        let mock_score_count = config.mock_score_count.unwrap_or(3).max(1);
        let mut provider_config = match config.provider_type.as_str() {
            astrbot_provider::MOCK_RERANK_PROVIDER_TYPE => {
                RerankProviderConfig::mock(config.id, vec![1.0_f32; mock_score_count])
            }
            astrbot_provider::VLLM_RERANK_PROVIDER_TYPE => RerankProviderConfig::vllm(
                config.id,
                config.api_base.unwrap_or_default(),
                config.model.unwrap_or_default(),
            ),
            astrbot_provider::BAILIAN_RERANK_PROVIDER_TYPE => RerankProviderConfig::bailian(
                config.id,
                config.api_base.unwrap_or_default(),
                config.model.unwrap_or_else(|| "gte-rerank-v2".to_string()),
            ),
            astrbot_provider::XINFERENCE_RERANK_PROVIDER_TYPE => RerankProviderConfig::xinference(
                config.id,
                config.api_base.unwrap_or_default(),
                config.model.unwrap_or_default(),
            ),
            _ => RerankProviderConfig {
                id: config.id,
                provider_type: config.provider_type,
                enabled: true,
                model: config.model,
                api_base: config.api_base,
                api_key: None,
                timeout: Duration::from_secs(config.timeout_secs),
                custom_headers: Default::default(),
                mock_scores: None,
                launch_model_if_not_running: config.launch_model_if_not_running,
            },
        };

        provider_config.enabled = config.enabled;
        provider_config.timeout = Duration::from_secs(config.timeout_secs);
        provider_config.api_key = config.api_key;
        provider_config.launch_model_if_not_running = config.launch_model_if_not_running;
        provider_config
    }
}
