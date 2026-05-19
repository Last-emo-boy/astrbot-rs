use std::sync::Arc;

use astrbot_core::Result;

use crate::config::SpeechToTextProviderConfig;
use crate::factories::common::{parse_provider_bool, provider_option};
use crate::{
    OpenAiSpeechToTextConfig, OpenAiSpeechToTextProvider, SelfhostSpeechToTextConfig,
    SelfhostSpeechToTextKind, SelfhostSpeechToTextProvider, SpeechToTextProvider,
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

pub(crate) fn build_openai_whisper_selfhost_speech_to_text_provider(
    config: &SpeechToTextProviderConfig,
) -> Result<Arc<dyn SpeechToTextProvider>> {
    build_selfhost_speech_to_text_provider(config, SelfhostSpeechToTextKind::OpenAiWhisper)
}

pub(crate) fn build_sensevoice_selfhost_speech_to_text_provider(
    config: &SpeechToTextProviderConfig,
) -> Result<Arc<dyn SpeechToTextProvider>> {
    build_selfhost_speech_to_text_provider(config, SelfhostSpeechToTextKind::SenseVoice)
}

fn build_selfhost_speech_to_text_provider(
    config: &SpeechToTextProviderConfig,
    kind: SelfhostSpeechToTextKind,
) -> Result<Arc<dyn SpeechToTextProvider>> {
    let api_base = config
        .api_base
        .as_deref()
        .map(str::trim)
        .filter(|api_base| !api_base.is_empty())
        .unwrap_or("http://127.0.0.1:8000")
        .trim_end_matches('/')
        .to_string();
    let model = config
        .model
        .clone()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| match kind {
            SelfhostSpeechToTextKind::OpenAiWhisper => "tiny".to_string(),
            SelfhostSpeechToTextKind::SenseVoice => "iic/SenseVoiceSmall".to_string(),
        });
    let mut speech_to_text_config =
        SelfhostSpeechToTextConfig::new(kind, api_base, model).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        speech_to_text_config = speech_to_text_config.with_api_key(api_key.clone());
    }
    if let Some(endpoint) = provider_option(&config.provider_options, &["endpoint", "stt_endpoint"])
    {
        speech_to_text_config = speech_to_text_config.with_endpoint(endpoint.to_string());
    }
    if let Some(proxy) = provider_option(&config.provider_options, &["proxy"]) {
        speech_to_text_config = speech_to_text_config.with_proxy(proxy.to_string());
    }
    if let Some(is_emotion) = provider_option(&config.provider_options, &["is_emotion"]) {
        speech_to_text_config =
            speech_to_text_config.with_emotion(parse_provider_bool(is_emotion, "is_emotion")?);
    }
    for (key, value) in &config.provider_options {
        if let Some(form_key) = key.strip_prefix("form_") {
            speech_to_text_config =
                speech_to_text_config.with_form_field(form_key.to_string(), value.clone());
        }
    }
    for (key, value) in &config.custom_headers {
        speech_to_text_config = speech_to_text_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(SelfhostSpeechToTextProvider::new(
        speech_to_text_config,
    )?))
}
