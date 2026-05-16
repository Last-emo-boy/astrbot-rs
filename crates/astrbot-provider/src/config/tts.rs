use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use crate::constants::{
    GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE, GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE,
    MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE, MOCK_TEXT_TO_SPEECH_PROVIDER_TYPE,
    OPENAI_TEXT_TO_SPEECH_PROVIDER_TYPE, VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE,
};

#[derive(Clone)]
pub struct TextToSpeechProviderConfig {
    pub id: String,
    pub provider_type: String,
    pub enabled: bool,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub mock_audio_path: Option<String>,
    pub supports_streaming: bool,
    pub voice: Option<String>,
    pub provider_options: HashMap<String, String>,
}

impl TextToSpeechProviderConfig {
    pub fn mock(id: impl Into<String>, audio_path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider_type: MOCK_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            mock_audio_path: Some(audio_path.into()),
            supports_streaming: false,
            voice: None,
            provider_options: HashMap::new(),
        }
    }

    pub fn openai(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: OPENAI_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            mock_audio_path: None,
            supports_streaming: false,
            voice: Some("alloy".to_string()),
            provider_options: HashMap::new(),
        }
    }

    pub fn gemini(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(20),
            custom_headers: HashMap::new(),
            mock_audio_path: None,
            supports_streaming: false,
            voice: Some("Leda".to_string()),
            provider_options: HashMap::new(),
        }
    }

    pub fn volcengine(id: impl Into<String>, api_base: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider_type: VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: None,
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(20),
            custom_headers: HashMap::new(),
            mock_audio_path: None,
            supports_streaming: false,
            voice: None,
            provider_options: HashMap::new(),
        }
    }

    pub fn minimax(
        id: impl Into<String>,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider_type: MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(60),
            custom_headers: HashMap::new(),
            mock_audio_path: None,
            supports_streaming: false,
            voice: None,
            provider_options: HashMap::new(),
        }
    }

    pub fn gsvi(id: impl Into<String>, api_base: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider_type: GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: None,
            api_base: Some(api_base.into()),
            api_key: None,
            timeout: Duration::from_secs(60),
            custom_headers: HashMap::new(),
            mock_audio_path: None,
            supports_streaming: false,
            voice: None,
            provider_options: HashMap::new(),
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

    pub fn with_streaming(mut self, supports_streaming: bool) -> Self {
        self.supports_streaming = supports_streaming;
        self
    }

    pub fn with_voice(mut self, voice: impl Into<String>) -> Self {
        self.voice = Some(voice.into());
        self
    }

    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.provider_options.insert(key.into(), value.into());
        self
    }
}

impl fmt::Debug for TextToSpeechProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextToSpeechProviderConfig")
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
                "mock_audio_path",
                &self.mock_audio_path.as_ref().map(|_| "<redacted>"),
            )
            .field("supports_streaming", &self.supports_streaming)
            .field("voice", &self.voice)
            .field(
                "provider_options",
                &self.provider_options.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}
