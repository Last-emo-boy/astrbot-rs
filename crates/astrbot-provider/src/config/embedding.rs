use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use crate::constants::{
    GEMINI_EMBEDDING_PROVIDER_TYPE, MOCK_EMBEDDING_PROVIDER_TYPE, OPENAI_EMBEDDING_PROVIDER_TYPE,
};

#[derive(Clone)]
pub struct EmbeddingProviderConfig {
    pub id: String,
    pub provider_type: String,
    pub enabled: bool,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub dimensions: Option<usize>,
    pub mock_embedding: Option<Vec<f32>>,
}

impl EmbeddingProviderConfig {
    pub fn mock(id: impl Into<String>, embedding: Vec<f32>) -> Self {
        Self {
            id: id.into(),
            provider_type: MOCK_EMBEDDING_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            dimensions: None,
            mock_embedding: Some(embedding),
        }
    }

    pub fn openai(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: OPENAI_EMBEDDING_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            dimensions: Some(1024),
            mock_embedding: None,
        }
    }

    pub fn gemini(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: GEMINI_EMBEDDING_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            dimensions: Some(768),
            mock_embedding: None,
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

    pub fn with_dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.insert(key.into(), value.into());
        self
    }
}

impl fmt::Debug for EmbeddingProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmbeddingProviderConfig")
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
            .field("dimensions", &self.dimensions)
            .field(
                "mock_embedding",
                &self
                    .mock_embedding
                    .as_ref()
                    .map(|embedding| embedding.len()),
            )
            .finish()
    }
}
