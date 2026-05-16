use std::collections::HashMap;
use std::time::Duration;

use astrbot_provider::TextToSpeechProviderConfig;
use serde::{Deserialize, Serialize};

use crate::defaults::{default_provider_timeout_secs, default_true};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeTextToSpeechProviderConfig {
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
    pub mock_audio_path: Option<String>,
    #[serde(default)]
    pub supports_streaming: bool,
    #[serde(default)]
    pub voice: Option<String>,
    #[serde(default)]
    pub provider_options: HashMap<String, String>,
}

impl RuntimeTextToSpeechProviderConfig {
    pub fn mock(id: impl Into<String>, audio_path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::MOCK_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: None,
            api_base: None,
            api_key: None,
            timeout_secs: 120,
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
            provider_type: astrbot_provider::OPENAI_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 120,
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
            provider_type: astrbot_provider::GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 20,
            mock_audio_path: None,
            supports_streaming: false,
            voice: Some("Leda".to_string()),
            provider_options: HashMap::new(),
        }
    }

    pub fn volcengine(id: impl Into<String>, api_base: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: None,
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 20,
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
            provider_type: astrbot_provider::MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: Some(model.into()),
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 60,
            mock_audio_path: None,
            supports_streaming: false,
            voice: None,
            provider_options: HashMap::new(),
        }
    }

    pub fn gsvi(id: impl Into<String>, api_base: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider_type: astrbot_provider::GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE.to_string(),
            enabled: true,
            model: None,
            api_base: Some(api_base.into()),
            api_key: None,
            timeout_secs: 60,
            mock_audio_path: None,
            supports_streaming: false,
            voice: None,
            provider_options: HashMap::new(),
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

impl From<RuntimeTextToSpeechProviderConfig> for TextToSpeechProviderConfig {
    fn from(config: RuntimeTextToSpeechProviderConfig) -> Self {
        let mut provider_config = match config.provider_type.as_str() {
            astrbot_provider::MOCK_TEXT_TO_SPEECH_PROVIDER_TYPE => {
                TextToSpeechProviderConfig::mock(
                    config.id,
                    config
                        .mock_audio_path
                        .unwrap_or_else(|| "mock.wav".to_string()),
                )
            }
            astrbot_provider::OPENAI_TEXT_TO_SPEECH_PROVIDER_TYPE => {
                TextToSpeechProviderConfig::openai(
                    config.id,
                    config.api_base.unwrap_or_default(),
                    config.model.unwrap_or_else(|| "tts-1".to_string()),
                )
            }
            astrbot_provider::GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE => {
                TextToSpeechProviderConfig::gemini(
                    config.id,
                    config
                        .api_base
                        .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string()),
                    config
                        .model
                        .unwrap_or_else(|| "gemini-2.5-flash-preview-tts".to_string()),
                )
            }
            astrbot_provider::VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE => {
                TextToSpeechProviderConfig::volcengine(
                    config.id,
                    config.api_base.unwrap_or_else(|| {
                        "https://openspeech.bytedance.com/api/v1/tts".to_string()
                    }),
                )
            }
            astrbot_provider::MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE => {
                TextToSpeechProviderConfig::minimax(
                    config.id,
                    config
                        .api_base
                        .unwrap_or_else(|| "https://api.minimax.chat/v1/t2a_v2".to_string()),
                    config.model.unwrap_or_default(),
                )
            }
            astrbot_provider::GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE => {
                TextToSpeechProviderConfig::gsvi(
                    config.id,
                    config
                        .api_base
                        .unwrap_or_else(|| "http://127.0.0.1:5000".to_string()),
                )
            }
            _ => TextToSpeechProviderConfig {
                id: config.id,
                provider_type: config.provider_type,
                enabled: true,
                model: config.model,
                api_base: config.api_base,
                api_key: None,
                timeout: Duration::from_secs(config.timeout_secs),
                custom_headers: Default::default(),
                mock_audio_path: config.mock_audio_path,
                supports_streaming: config.supports_streaming,
                voice: config.voice.clone(),
                provider_options: config.provider_options.clone(),
            },
        };

        provider_config.enabled = config.enabled;
        provider_config.timeout = Duration::from_secs(config.timeout_secs);
        provider_config.api_key = config.api_key;
        provider_config.supports_streaming = config.supports_streaming;
        if let Some(voice) = config.voice {
            provider_config.voice = Some(voice);
        }
        provider_config.provider_options = config.provider_options;
        provider_config
    }
}
