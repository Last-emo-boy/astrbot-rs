use std::sync::Arc;

use astrbot_core::Result;

use super::options;
use crate::config::TextToSpeechProviderConfig;
use crate::{GsvSelfhostTextToSpeechConfig, GsvSelfhostTextToSpeechProvider, TextToSpeechProvider};

pub(crate) fn build_gsv_selfhost_text_to_speech_provider(
    config: &TextToSpeechProviderConfig,
) -> Result<Arc<dyn TextToSpeechProvider>> {
    let api_base = config
        .api_base
        .as_deref()
        .map(str::trim)
        .filter(|api_base| !api_base.is_empty())
        .unwrap_or("http://127.0.0.1:9880")
        .trim_end_matches('/')
        .to_string();
    let mut text_to_speech_config =
        GsvSelfhostTextToSpeechConfig::new(api_base).with_timeout(config.timeout);

    if let Some(path) = options::option(&config.provider_options, &["gpt_weights_path"]) {
        text_to_speech_config = text_to_speech_config.with_gpt_weights_path(path.to_string());
    }
    if let Some(path) = options::option(&config.provider_options, &["sovits_weights_path"]) {
        text_to_speech_config = text_to_speech_config.with_sovits_weights_path(path.to_string());
    }
    if let Some(proxy) = options::option(&config.provider_options, &["proxy"]) {
        text_to_speech_config = text_to_speech_config.with_proxy(proxy.to_string());
    }
    for (key, value) in &config.provider_options {
        if is_gsv_default_param_key(key) {
            text_to_speech_config =
                text_to_speech_config.with_default_param(key.clone(), value.clone());
        }
    }
    for (key, value) in &config.custom_headers {
        text_to_speech_config = text_to_speech_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(GsvSelfhostTextToSpeechProvider::new(
        text_to_speech_config,
    )?))
}

fn is_gsv_default_param_key(key: &str) -> bool {
    key.starts_with("gsv_") || key.starts_with("gsv_default_parms.")
}
