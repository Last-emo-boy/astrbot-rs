use std::sync::Arc;

use astrbot_core::Result;

use crate::config::TextToSpeechProviderConfig;
use crate::{GeminiTextToSpeechConfig, GeminiTextToSpeechProvider, TextToSpeechProvider};

pub(crate) fn build_gemini_text_to_speech_provider(
    config: &TextToSpeechProviderConfig,
) -> Result<Arc<dyn TextToSpeechProvider>> {
    let api_base = config
        .api_base
        .as_deref()
        .map(str::trim)
        .filter(|api_base| !api_base.is_empty())
        .unwrap_or("https://generativelanguage.googleapis.com")
        .trim_end_matches('/')
        .to_string();
    let model = config
        .model
        .clone()
        .unwrap_or_else(|| "gemini-2.5-flash-preview-tts".to_string());
    let mut text_to_speech_config =
        GeminiTextToSpeechConfig::new(api_base, model).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        text_to_speech_config = text_to_speech_config.with_api_key(api_key.clone());
    }
    if let Some(voice) = &config.voice {
        text_to_speech_config = text_to_speech_config.with_voice(voice.clone());
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(GeminiTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}
