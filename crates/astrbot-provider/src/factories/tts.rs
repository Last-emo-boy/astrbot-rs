use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};

use super::common::{parse_provider_bool, parse_provider_f32, provider_option};
use crate::config::TextToSpeechProviderConfig;
use crate::{
    GeminiTextToSpeechConfig, GeminiTextToSpeechProvider, GsviTextToSpeechConfig,
    GsviTextToSpeechProvider, MiniMaxTextToSpeechConfig, MiniMaxTextToSpeechProvider,
    OpenAiTextToSpeechConfig, OpenAiTextToSpeechProvider, TextToSpeechProvider,
    VolcengineTextToSpeechConfig, VolcengineTextToSpeechProvider,
};

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
    if let Some(appid) = provider_option(&config.provider_options, &["appid", "app_id"]) {
        text_to_speech_config = text_to_speech_config.with_appid(appid.to_string());
    }
    if let Some(cluster) =
        provider_option(&config.provider_options, &["volcengine_cluster", "cluster"])
    {
        text_to_speech_config = text_to_speech_config.with_cluster(cluster.to_string());
    }
    if let Some(voice_type) = provider_option(
        &config.provider_options,
        &["volcengine_voice_type", "voice_type"],
    )
    .or(config.voice.as_deref())
    {
        text_to_speech_config = text_to_speech_config.with_voice_type(voice_type.to_string());
    }
    if let Some(speed_ratio) = provider_option(
        &config.provider_options,
        &["volcengine_speed_ratio", "speed_ratio"],
    ) {
        let speed_ratio = speed_ratio.parse::<f32>().map_err(|_| {
            AstrbotError::Provider(format!(
                "invalid Volcengine TTS speed_ratio option: {speed_ratio}"
            ))
        })?;
        text_to_speech_config = text_to_speech_config.with_speed_ratio(speed_ratio);
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(VolcengineTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}

pub(crate) fn build_minimax_text_to_speech_provider(
    config: &TextToSpeechProviderConfig,
) -> Result<Arc<dyn TextToSpeechProvider>> {
    let api_base = config
        .api_base
        .as_deref()
        .map(str::trim)
        .filter(|api_base| !api_base.is_empty())
        .unwrap_or("https://api.minimax.chat/v1/t2a_v2")
        .to_string();
    let model = config.model.clone().unwrap_or_default();
    let mut text_to_speech_config =
        MiniMaxTextToSpeechConfig::new(api_base, model).with_timeout(config.timeout);

    if let Some(api_key) = &config.api_key {
        text_to_speech_config = text_to_speech_config.with_api_key(api_key.clone());
    }
    if let Some(group_id) = provider_option(
        &config.provider_options,
        &["minimax-group-id", "group_id", "group-id"],
    ) {
        text_to_speech_config = text_to_speech_config.with_group_id(group_id.to_string());
    }
    if let Some(language_boost) = provider_option(
        &config.provider_options,
        &["minimax-langboost", "language_boost", "langboost"],
    ) {
        text_to_speech_config =
            text_to_speech_config.with_language_boost(language_boost.to_string());
    }
    if let Some(is_timber_weight) = provider_option(
        &config.provider_options,
        &["minimax-is-timber-weight", "is_timber_weight"],
    ) {
        text_to_speech_config = text_to_speech_config.with_timber_weight_enabled(
            parse_provider_bool(is_timber_weight, "minimax-is-timber-weight")?,
        );
    }
    if let Some(timber_weights) = provider_option(
        &config.provider_options,
        &["minimax-timber-weight", "timber_weights"],
    ) {
        let timber_weights =
            serde_json::from_str::<serde_json::Value>(timber_weights).map_err(|err| {
                AstrbotError::Provider(format!("invalid MiniMax TTS timber_weights JSON: {err}"))
            })?;
        text_to_speech_config = text_to_speech_config.with_timber_weights(timber_weights);
    }
    if let Some(speed) = provider_option(
        &config.provider_options,
        &["minimax-voice-speed", "voice_speed"],
    ) {
        text_to_speech_config = text_to_speech_config
            .with_voice_speed(parse_provider_f32(speed, "minimax-voice-speed")?);
    }
    if let Some(volume) = provider_option(
        &config.provider_options,
        &["minimax-voice-vol", "voice_volume"],
    ) {
        text_to_speech_config = text_to_speech_config
            .with_voice_volume(parse_provider_f32(volume, "minimax-voice-vol")?);
    }
    if let Some(pitch) = provider_option(
        &config.provider_options,
        &["minimax-voice-pitch", "voice_pitch"],
    ) {
        text_to_speech_config = text_to_speech_config
            .with_voice_pitch(parse_provider_f32(pitch, "minimax-voice-pitch")?);
    }
    if let Some(voice_id) = provider_option(&config.provider_options, &["minimax-voice-id"]) {
        text_to_speech_config = text_to_speech_config.with_voice_id(voice_id.to_string());
    } else if let Some(voice) = &config.voice {
        text_to_speech_config = text_to_speech_config.with_voice_id(voice.clone());
    }
    if let Some(emotion) = provider_option(
        &config.provider_options,
        &["minimax-voice-emotion", "voice_emotion"],
    ) {
        text_to_speech_config = text_to_speech_config.with_voice_emotion(emotion.to_string());
    }
    if let Some(latex_read) = provider_option(
        &config.provider_options,
        &["minimax-voice-latex", "latex_read"],
    ) {
        text_to_speech_config = text_to_speech_config
            .with_voice_latex_read(parse_provider_bool(latex_read, "minimax-voice-latex")?);
    }
    if let Some(english_normalization) = provider_option(
        &config.provider_options,
        &[
            "minimax-voice-english-normalization",
            "english_normalization",
        ],
    ) {
        text_to_speech_config = text_to_speech_config.with_voice_english_normalization(
            parse_provider_bool(english_normalization, "minimax-voice-english-normalization")?,
        );
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(MiniMaxTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}

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

    if let Some(character) = provider_option(&config.provider_options, &["character"]) {
        text_to_speech_config = text_to_speech_config.with_character(character.to_string());
    } else if let Some(voice) = &config.voice {
        text_to_speech_config = text_to_speech_config.with_character(voice.clone());
    }
    if let Some(emotion) = provider_option(&config.provider_options, &["emotion"]) {
        text_to_speech_config = text_to_speech_config.with_emotion(emotion.to_string());
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(GsviTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}
