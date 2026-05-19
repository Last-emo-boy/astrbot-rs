use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};

use super::options;
use crate::config::TextToSpeechProviderConfig;
use crate::{
    AzureCommercialTextToSpeechProvider, AzureOttsTextToSpeechConfig, AzureTextToSpeechConfig,
    DashscopeTextToSpeechConfig, DashscopeTextToSpeechProvider, DashscopeTtsMode,
    EdgeTextToSpeechConfig, EdgeTextToSpeechProvider, FishAudioTextToSpeechConfig,
    FishAudioTextToSpeechProvider, GenieTextToSpeechConfig, GenieTextToSpeechProvider,
    TextToSpeechProvider,
};

pub(crate) fn build_azure_text_to_speech_provider(
    config: &TextToSpeechProviderConfig,
) -> Result<Arc<dyn TextToSpeechProvider>> {
    let subscription_key = options::option(
        &config.provider_options,
        &["azure_tts_subscription_key", "subscription_key"],
    )
    .or(config.api_key.as_deref())
    .unwrap_or_default();

    let mut native = AzureTextToSpeechConfig::new(subscription_key).with_timeout(config.timeout);
    if let Some(region) = options::option(&config.provider_options, &["azure_tts_region", "region"])
    {
        native = native.with_region(region.to_string());
    }
    if let Some(voice) = config
        .voice
        .as_deref()
        .or_else(|| options::option(&config.provider_options, &["azure_tts_voice", "voice"]))
    {
        native = native.with_voice(voice.to_string());
    }
    if let Some(style) = options::option(&config.provider_options, &["azure_tts_style", "style"]) {
        native = native.with_style(style.to_string());
    }
    if let Some(role) = options::option(&config.provider_options, &["azure_tts_role", "role"]) {
        native = native.with_role(role.to_string());
    }
    if let Some(rate) = options::option(&config.provider_options, &["azure_tts_rate", "rate"]) {
        native = native.with_rate(rate.to_string());
    }
    if let Some(volume) = options::option(&config.provider_options, &["azure_tts_volume", "volume"])
    {
        native = native.with_volume(volume.to_string());
    }
    if let Some(endpoint) = config.api_base.as_deref().or_else(|| {
        options::option(
            &config.provider_options,
            &["azure_tts_endpoint", "endpoint", "tts_endpoint"],
        )
    }) {
        native = native.with_endpoint_override(endpoint.to_string());
    }
    if let Some(token_url) = options::option(
        &config.provider_options,
        &["azure_tts_token_url", "token_url"],
    ) {
        native = native.with_token_url_override(token_url.to_string());
    }
    if let Some(proxy) = options::option(&config.provider_options, &["proxy"]) {
        native = native.with_proxy(proxy.to_string());
    }
    for (key, value) in &config.custom_headers {
        native = native.with_header(key.clone(), value.clone());
    }

    let otts = parse_otts_config(subscription_key, config, &native)?;
    Ok(Arc::new(AzureCommercialTextToSpeechProvider::from_configs(
        native, otts,
    )?))
}

pub(crate) fn build_edge_text_to_speech_provider(
    config: &TextToSpeechProviderConfig,
) -> Result<Arc<dyn TextToSpeechProvider>> {
    let api_base = config
        .api_base
        .as_deref()
        .map(str::trim)
        .filter(|api_base| !api_base.is_empty())
        .unwrap_or("http://127.0.0.1:8765")
        .trim_end_matches('/')
        .to_string();
    let mut text_to_speech_config =
        EdgeTextToSpeechConfig::new(api_base).with_timeout(config.timeout);
    if let Some(voice) = options::option(&config.provider_options, &["edge-tts-voice", "voice"])
        .or(config.voice.as_deref())
    {
        text_to_speech_config = text_to_speech_config.with_voice(voice.to_string());
    }
    if let Some(rate) = options::option(&config.provider_options, &["rate", "edge_tts_rate"]) {
        text_to_speech_config = text_to_speech_config.with_rate(rate.to_string());
    }
    if let Some(volume) = options::option(&config.provider_options, &["volume", "edge_tts_volume"])
    {
        text_to_speech_config = text_to_speech_config.with_volume(volume.to_string());
    }
    if let Some(pitch) = options::option(&config.provider_options, &["pitch", "edge_tts_pitch"]) {
        text_to_speech_config = text_to_speech_config.with_pitch(pitch.to_string());
    }
    if let Some(proxy) = options::option(&config.provider_options, &["proxy"]) {
        text_to_speech_config = text_to_speech_config.with_proxy(proxy.to_string());
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(EdgeTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}

pub(crate) fn build_dashscope_text_to_speech_provider(
    config: &TextToSpeechProviderConfig,
) -> Result<Arc<dyn TextToSpeechProvider>> {
    let api_base = config
        .api_base
        .as_deref()
        .map(str::trim)
        .filter(|api_base| !api_base.is_empty())
        .unwrap_or("https://dashscope.aliyuncs.com/api/v1/services/aigc")
        .trim_end_matches('/')
        .to_string();
    let model = config.model.clone().unwrap_or_default();
    let mut text_to_speech_config =
        DashscopeTextToSpeechConfig::new(api_base, model).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        text_to_speech_config = text_to_speech_config.with_api_key(api_key.clone());
    }
    if let Some(voice) =
        options::option(&config.provider_options, &["dashscope_tts_voice", "voice"])
            .or(config.voice.as_deref())
    {
        text_to_speech_config = text_to_speech_config.with_voice(voice.to_string());
    }
    if let Some(mode) = options::option(&config.provider_options, &["dashscope_tts_mode", "mode"]) {
        let mode = DashscopeTtsMode::parse(mode)
            .ok_or_else(|| AstrbotError::Provider(format!("invalid Dashscope TTS mode: {mode}")))?;
        text_to_speech_config = text_to_speech_config.with_mode(mode);
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(DashscopeTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}

pub(crate) fn build_fishaudio_text_to_speech_provider(
    config: &TextToSpeechProviderConfig,
) -> Result<Arc<dyn TextToSpeechProvider>> {
    let api_base = config
        .api_base
        .as_deref()
        .map(str::trim)
        .filter(|api_base| !api_base.is_empty())
        .unwrap_or("https://api.fish-audio.cn/v1")
        .trim_end_matches('/')
        .to_string();
    let mut text_to_speech_config =
        FishAudioTextToSpeechConfig::new(api_base).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        text_to_speech_config = text_to_speech_config.with_api_key(api_key.clone());
    }
    if let Some(reference_id) = options::option(
        &config.provider_options,
        &["fishaudio-tts-reference-id", "reference_id"],
    ) {
        text_to_speech_config = text_to_speech_config.with_reference_id(reference_id.to_string());
    }
    if let Some(character) = options::option(
        &config.provider_options,
        &["fishaudio-tts-character", "character"],
    )
    .or(config.voice.as_deref())
    {
        text_to_speech_config = text_to_speech_config.with_character(character.to_string());
    }
    if let Some(model) = config.model.as_deref() {
        text_to_speech_config = text_to_speech_config.with_model(model.to_string());
    }
    if let Some(proxy) = options::option(&config.provider_options, &["proxy"]) {
        text_to_speech_config = text_to_speech_config.with_proxy(proxy.to_string());
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(FishAudioTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}

pub(crate) fn build_genie_text_to_speech_provider(
    config: &TextToSpeechProviderConfig,
) -> Result<Arc<dyn TextToSpeechProvider>> {
    let api_base = config
        .api_base
        .as_deref()
        .map(str::trim)
        .filter(|api_base| !api_base.is_empty())
        .unwrap_or("http://127.0.0.1:8766")
        .trim_end_matches('/')
        .to_string();
    let mut text_to_speech_config =
        GenieTextToSpeechConfig::new(api_base).with_timeout(config.timeout);
    if let Some(character_name) = options::option(
        &config.provider_options,
        &["genie_character_name", "character_name"],
    )
    .or(config.voice.as_deref())
    {
        text_to_speech_config =
            text_to_speech_config.with_character_name(character_name.to_string());
    }
    if let Some(language) =
        options::option(&config.provider_options, &["genie_language", "language"])
    {
        text_to_speech_config = text_to_speech_config.with_language(language.to_string());
    }
    if let Some(model_dir) = options::option(
        &config.provider_options,
        &["genie_onnx_model_dir", "onnx_model_dir"],
    ) {
        text_to_speech_config = text_to_speech_config.with_onnx_model_dir(model_dir.to_string());
    }
    if let Some(refer_audio_path) = options::option(
        &config.provider_options,
        &["genie_refer_audio_path", "refer_audio_path"],
    ) {
        text_to_speech_config =
            text_to_speech_config.with_refer_audio_path(refer_audio_path.to_string());
    }
    if let Some(refer_text) = options::option(
        &config.provider_options,
        &["genie_refer_text", "refer_text"],
    ) {
        text_to_speech_config = text_to_speech_config.with_refer_text(refer_text.to_string());
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(GenieTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}

fn parse_otts_config(
    subscription_key: &str,
    config: &TextToSpeechProviderConfig,
    native: &AzureTextToSpeechConfig,
) -> Result<Option<AzureOttsTextToSpeechConfig>> {
    let Some(json) = subscription_key
        .trim()
        .strip_prefix("other[")
        .and_then(|value| value.strip_suffix(']'))
    else {
        return Ok(None);
    };
    let value = serde_json::from_str::<serde_json::Value>(json)
        .map_err(|err| AstrbotError::Provider(format!("invalid Azure OTTS JSON: {err}")))?;
    let skey = value
        .get("OTTS_SKEY")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let api_url = value
        .get("OTTS_URL")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let auth_time_url = value
        .get("OTTS_AUTH_TIME")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let mut otts =
        AzureOttsTextToSpeechConfig::new(skey, api_url, auth_time_url).with_timeout(config.timeout);
    otts = otts
        .with_voice(native.voice.clone())
        .with_style(native.style.clone())
        .with_role(native.role.clone())
        .with_rate(native.rate.clone())
        .with_volume(native.volume.clone());
    if let Some(proxy) = options::option(&config.provider_options, &["proxy"])
        .or_else(|| value.get("proxy").and_then(|value| value.as_str()))
    {
        otts = otts.with_proxy(proxy.to_string());
    }
    for (key, value) in &config.custom_headers {
        otts = otts.with_header(key.clone(), value.clone());
    }
    Ok(Some(otts))
}
