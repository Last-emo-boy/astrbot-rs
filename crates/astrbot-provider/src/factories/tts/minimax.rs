use std::sync::Arc;

use astrbot_core::Result;

use super::options;
use crate::config::TextToSpeechProviderConfig;
use crate::{MiniMaxTextToSpeechConfig, MiniMaxTextToSpeechProvider, TextToSpeechProvider};

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
    if let Some(group_id) = options::option(
        &config.provider_options,
        &["minimax-group-id", "group_id", "group-id"],
    ) {
        text_to_speech_config = text_to_speech_config.with_group_id(group_id.to_string());
    }
    if let Some(language_boost) = options::option(
        &config.provider_options,
        &["minimax-langboost", "language_boost", "langboost"],
    ) {
        text_to_speech_config =
            text_to_speech_config.with_language_boost(language_boost.to_string());
    }
    if let Some(is_timber_weight) = options::option(
        &config.provider_options,
        &["minimax-is-timber-weight", "is_timber_weight"],
    ) {
        text_to_speech_config = text_to_speech_config.with_timber_weight_enabled(
            options::bool_option(is_timber_weight, "minimax-is-timber-weight")?,
        );
    }
    if let Some(timber_weights) = options::option(
        &config.provider_options,
        &["minimax-timber-weight", "timber_weights"],
    ) {
        let timber_weights =
            options::json_option(timber_weights, "invalid MiniMax TTS timber_weights JSON")?;
        text_to_speech_config = text_to_speech_config.with_timber_weights(timber_weights);
    }
    if let Some(speed) = options::option(
        &config.provider_options,
        &["minimax-voice-speed", "voice_speed"],
    ) {
        text_to_speech_config = text_to_speech_config
            .with_voice_speed(options::f32_option(speed, "minimax-voice-speed")?);
    }
    if let Some(volume) = options::option(
        &config.provider_options,
        &["minimax-voice-vol", "voice_volume"],
    ) {
        text_to_speech_config = text_to_speech_config
            .with_voice_volume(options::f32_option(volume, "minimax-voice-vol")?);
    }
    if let Some(pitch) = options::option(
        &config.provider_options,
        &["minimax-voice-pitch", "voice_pitch"],
    ) {
        text_to_speech_config = text_to_speech_config
            .with_voice_pitch(options::f32_option(pitch, "minimax-voice-pitch")?);
    }
    if let Some(voice_id) = options::option(&config.provider_options, &["minimax-voice-id"]) {
        text_to_speech_config = text_to_speech_config.with_voice_id(voice_id.to_string());
    } else if let Some(voice) = &config.voice {
        text_to_speech_config = text_to_speech_config.with_voice_id(voice.clone());
    }
    if let Some(emotion) = options::option(
        &config.provider_options,
        &["minimax-voice-emotion", "voice_emotion"],
    ) {
        text_to_speech_config = text_to_speech_config.with_voice_emotion(emotion.to_string());
    }
    if let Some(latex_read) = options::option(
        &config.provider_options,
        &["minimax-voice-latex", "latex_read"],
    ) {
        text_to_speech_config = text_to_speech_config
            .with_voice_latex_read(options::bool_option(latex_read, "minimax-voice-latex")?);
    }
    if let Some(english_normalization) = options::option(
        &config.provider_options,
        &[
            "minimax-voice-english-normalization",
            "english_normalization",
        ],
    ) {
        text_to_speech_config = text_to_speech_config.with_voice_english_normalization(
            options::bool_option(english_normalization, "minimax-voice-english-normalization")?,
        );
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(MiniMaxTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}
