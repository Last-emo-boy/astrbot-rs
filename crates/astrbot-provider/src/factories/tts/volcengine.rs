use std::sync::Arc;

use astrbot_core::Result;

use super::options;
use crate::config::TextToSpeechProviderConfig;
use crate::{TextToSpeechProvider, VolcengineTextToSpeechConfig, VolcengineTextToSpeechProvider};

pub(crate) fn build_volcengine_text_to_speech_provider(
    config: &TextToSpeechProviderConfig,
) -> Result<Arc<dyn TextToSpeechProvider>> {
    let api_base = config
        .api_base
        .as_deref()
        .map(str::trim)
        .filter(|api_base| !api_base.is_empty())
        .unwrap_or("https://openspeech.bytedance.com/api/v1/tts")
        .to_string();
    let mut text_to_speech_config =
        VolcengineTextToSpeechConfig::new(api_base).with_timeout(config.timeout);

    if let Some(api_key) = &config.api_key {
        text_to_speech_config = text_to_speech_config.with_api_key(api_key.clone());
    }
    if let Some(appid) = options::option(&config.provider_options, &["appid", "app_id"]) {
        text_to_speech_config = text_to_speech_config.with_appid(appid.to_string());
    }
    if let Some(cluster) =
        options::option(&config.provider_options, &["volcengine_cluster", "cluster"])
    {
        text_to_speech_config = text_to_speech_config.with_cluster(cluster.to_string());
    }
    if let Some(voice_type) = options::option(
        &config.provider_options,
        &["volcengine_voice_type", "voice_type"],
    )
    .or(config.voice.as_deref())
    {
        text_to_speech_config = text_to_speech_config.with_voice_type(voice_type.to_string());
    }
    if let Some(speed_ratio) = options::option(
        &config.provider_options,
        &["volcengine_speed_ratio", "speed_ratio"],
    ) {
        let speed_ratio =
            options::named_f32_option(speed_ratio, "invalid Volcengine TTS speed_ratio option")?;
        text_to_speech_config = text_to_speech_config.with_speed_ratio(speed_ratio);
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(VolcengineTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}
