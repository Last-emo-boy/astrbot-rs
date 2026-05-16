use std::sync::Arc;

use astrbot_core::Result;

use super::options;
use crate::config::TextToSpeechProviderConfig;
use crate::{GsviTextToSpeechConfig, GsviTextToSpeechProvider, TextToSpeechProvider};

pub(crate) fn build_gsvi_text_to_speech_provider(
    config: &TextToSpeechProviderConfig,
) -> Result<Arc<dyn TextToSpeechProvider>> {
    let api_base = config
        .api_base
        .as_deref()
        .map(str::trim)
        .filter(|api_base| !api_base.is_empty())
        .unwrap_or("http://127.0.0.1:5000")
        .to_string();
    let mut text_to_speech_config =
        GsviTextToSpeechConfig::new(api_base).with_timeout(config.timeout);

    if let Some(character) = options::option(&config.provider_options, &["character"]) {
        text_to_speech_config = text_to_speech_config.with_character(character.to_string());
    } else if let Some(voice) = &config.voice {
        text_to_speech_config = text_to_speech_config.with_character(voice.clone());
    }
    if let Some(emotion) = options::option(&config.provider_options, &["emotion"]) {
        text_to_speech_config = text_to_speech_config.with_emotion(emotion.to_string());
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(GsviTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}
