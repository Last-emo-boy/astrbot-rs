use std::sync::Arc;

use astrbot_core::Result;

use crate::config::SpeechToTextProviderConfig;
use crate::{
    OpenAiSpeechToTextConfig, OpenAiSpeechToTextProvider, SpeechToTextProvider,
    XinferenceSpeechToTextConfig, XinferenceSpeechToTextProvider,
};

pub(crate) fn build_openai_speech_to_text_provider(
    config: &SpeechToTextProviderConfig,
) -> Result<Arc<dyn SpeechToTextProvider>> {
    let api_base = normalize_openai_speech_to_text_api_base(config.api_base.as_deref());
    let model = config
        .model
        .clone()
        .unwrap_or_else(|| "whisper-1".to_string());
    let mut speech_to_text_config =
        OpenAiSpeechToTextConfig::new(api_base, model).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        speech_to_text_config = speech_to_text_config.with_api_key(api_key.clone());
    }
    for (key, value) in &config.custom_headers {
        speech_to_text_config = speech_to_text_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(OpenAiSpeechToTextProvider::new(
        speech_to_text_config,
    )?))
}

fn normalize_openai_speech_to_text_api_base(api_base: Option<&str>) -> String {
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

pub(crate) fn build_xinference_speech_to_text_provider(
    config: &SpeechToTextProviderConfig,
) -> Result<Arc<dyn SpeechToTextProvider>> {
    let api_base = config
        .api_base
        .as_deref()
        .map(str::trim)
        .filter(|api_base| !api_base.is_empty())
        .unwrap_or("http://127.0.0.1:9997")
        .trim_end_matches('/')
        .to_string();
    let model = config
        .model
        .clone()
        .unwrap_or_else(|| "whisper-large-v3".to_string());
    let mut speech_to_text_config = XinferenceSpeechToTextConfig::new(api_base, model)
        .with_timeout(config.timeout)
        .with_launch_model_if_not_running(config.launch_model_if_not_running);
    if let Some(api_key) = &config.api_key {
        speech_to_text_config = speech_to_text_config.with_api_key(api_key.clone());
    }
    for (key, value) in &config.custom_headers {
        speech_to_text_config = speech_to_text_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(XinferenceSpeechToTextProvider::new(
        speech_to_text_config,
    )?))
}
