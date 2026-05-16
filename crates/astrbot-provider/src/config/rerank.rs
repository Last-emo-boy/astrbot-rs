use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use crate::constants::{
    BAILIAN_RERANK_PROVIDER_TYPE, MOCK_RERANK_PROVIDER_TYPE, VLLM_RERANK_PROVIDER_TYPE,
    XINFERENCE_RERANK_PROVIDER_TYPE,
};

#[derive(Clone)]
pub struct RerankProviderConfig {
    pub id: String,
    pub provider_type: String,
    pub enabled: bool,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub mock_scores: Option<Vec<f32>>,
    pub launch_model_if_not_running: bool,
}

impl RerankProviderConfig {
    pub fn mock(id: impl Into<String>, scores: Vec<f32>) -> Self {
        Self {
            id: id.into(),
            provider_type: MOCK_RERANK_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            mock_scores: Some(scores),
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
            provider_type: VLLM_RERANK_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            mock_scores: None,
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
            provider_type: BAILIAN_RERANK_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            mock_scores: None,
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
            provider_type: XINFERENCE_RERANK_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            mock_scores: None,
            launch_model_if_not_running: false,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.insert(key.into(), value.into());
        self
    }

    pub fn with_launch_model_if_not_running(mut self, launch_model_if_not_running: bool) -> Self {
        self.launch_model_if_not_running = launch_model_if_not_running;
        self
    }
}

impl fmt::Debug for RerankProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RerankProviderConfig")
            .field("id", &self.id)
            .field("provider_type", &self.provider_type)
            .field("enabled", &self.enabled)
            .field("model", &self.model)
            .field("api_base", &self.api_base)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("timeout", &self.timeout)
            .field(
                "custom_headers",
                &self.custom_headers.keys().collect::<Vec<_>>(),
            )
            .field(
                "mock_scores",
                &self.mock_scores.as_ref().map(|scores| scores.len()),
            )
            .field(
                "launch_model_if_not_running",
                &self.launch_model_if_not_running,
            )
            .finish()
    }
}
