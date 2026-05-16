use std::sync::Arc;

use astrbot_core::Result;

use crate::config::TextToSpeechProviderConfig;
use crate::{OpenAiTextToSpeechConfig, OpenAiTextToSpeechProvider, TextToSpeechProvider};

pub(crate) fn build_openai_text_to_speech_provider(
    config: &TextToSpeechProviderConfig,
) -> Result<Arc<dyn TextToSpeechProvider>> {
    let api_base = normalize_openai_text_to_speech_api_base(config.api_base.as_deref());
    let model = config.model.clone().unwrap_or_else(|| "tts-1".to_string());
    let mut text_to_speech_config =
        OpenAiTextToSpeechConfig::new(api_base, model).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        text_to_speech_config = text_to_speech_config.with_api_key(api_key.clone());
    }
    if let Some(voice) = &config.voice {
        text_to_speech_config = text_to_speech_config.with_voice(voice.clone());
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(OpenAiTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}

fn normalize_openai_text_to_speech_api_base(api_base: Option<&str>) -> String {
    let api_base = api_base
        .map(str::trim)
        .filter(|api_base| !api_base.is_empty())
        .unwrap_or("https://api.openai.com/v1")
        .trim_end_matches('/')
        .to_string();

    if api_base.ends_with("/v1") {
        api_base
    } else {
        format!("{api_base}/v1")
    }
}
