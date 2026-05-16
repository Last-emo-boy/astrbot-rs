use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use crate::constants::{
    ANTHROPIC_CHAT_PROVIDER_TYPE, GOOGLE_GENAI_CHAT_PROVIDER_TYPE, MOCK_CHAT_PROVIDER_TYPE,
    OPENAI_CHAT_PROVIDER_TYPE,
};

#[derive(Clone)]
pub struct ChatProviderConfig {
    pub id: String,
    pub provider_type: String,
    pub enabled: bool,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub mock_response: Option<String>,
}

impl ChatProviderConfig {
    pub fn mock(id: impl Into<String>, response: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider_type: MOCK_CHAT_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
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
            provider_type: OPENAI_CHAT_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
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
            provider_type: ANTHROPIC_CHAT_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
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
            provider_type: GOOGLE_GENAI_CHAT_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            mock_response: None,
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
}

impl fmt::Debug for ChatProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatProviderConfig")
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
                "mock_response",
                &self.mock_response.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}
