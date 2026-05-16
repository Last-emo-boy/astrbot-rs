use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};

use crate::config::ChatProviderConfig;
use crate::registry::ProviderRegistry;
use crate::{
    AnthropicConfig, AnthropicProvider, ChatProvider, GeminiConfig, GeminiProvider,
    OpenAiCompatibleConfig, OpenAiCompatibleProvider,
};

#[derive(Clone, Copy)]
pub(crate) enum OpenAiCompatiblePreset {
    Default,
    OpenRouter,
    AiHubMix,
}

pub(crate) fn register_openai_compatible_alias(
    registry: &mut ProviderRegistry,
    provider_type: &'static str,
    preset: OpenAiCompatiblePreset,
) {
    registry
        .register_chat_provider(provider_type, move |config| {
            build_openai_compatible_provider(config, preset)
        })
        .expect("built-in OpenAI-compatible provider alias should register once");
}

pub(crate) fn build_openai_compatible_provider(
    config: &ChatProviderConfig,
    preset: OpenAiCompatiblePreset,
) -> Result<Arc<dyn ChatProvider>> {
    let api_base = config.api_base.clone().ok_or_else(|| {
        AstrbotError::Provider(format!("provider {} missing api_base", config.id))
    })?;
    let model = config
        .model
        .clone()
        .ok_or_else(|| AstrbotError::Provider(format!("provider {} missing model", config.id)))?;
    let mut openai_config =
        OpenAiCompatibleConfig::new(api_base, model).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        openai_config = openai_config.with_api_key(api_key.clone());
    }

    let mut headers = config.custom_headers.clone();
    match preset {
        OpenAiCompatiblePreset::Default => {}
        OpenAiCompatiblePreset::OpenRouter => {
            headers
                .entry("HTTP-Referer".to_string())
                .or_insert_with(|| "https://github.com/AstrBotDevs/AstrBot".to_string());
            headers
                .entry("X-TITLE".to_string())
                .or_insert_with(|| "AstrBot".to_string());
        }
        OpenAiCompatiblePreset::AiHubMix => {
            headers
                .entry("APP-Code".to_string())
                .or_insert_with(|| "KRLC5702".to_string());
        }
    }

    for (key, value) in headers {
        openai_config = openai_config.with_header(key, value);
    }
    Ok(Arc::new(OpenAiCompatibleProvider::new(openai_config)?))
}

pub(crate) fn build_anthropic_provider(
    config: &ChatProviderConfig,
) -> Result<Arc<dyn ChatProvider>> {
    let api_base = config
        .api_base
        .clone()
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());
    let model = config
        .model
        .clone()
        .ok_or_else(|| AstrbotError::Provider(format!("provider {} missing model", config.id)))?;
    let mut anthropic_config = AnthropicConfig::new(api_base, model).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        anthropic_config = anthropic_config.with_api_key(api_key.clone());
    }
    for (key, value) in &config.custom_headers {
        anthropic_config = anthropic_config.with_header(key.clone(), value.clone());
    }
    Ok(Arc::new(AnthropicProvider::new(anthropic_config)?))
}

pub(crate) fn build_gemini_provider(config: &ChatProviderConfig) -> Result<Arc<dyn ChatProvider>> {
    let api_base = config
        .api_base
        .clone()
        .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string());
    let model = config
        .model
        .clone()
        .ok_or_else(|| AstrbotError::Provider(format!("provider {} missing model", config.id)))?;
    let mut gemini_config = GeminiConfig::new(api_base, model).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        gemini_config = gemini_config.with_api_key(api_key.clone());
    }
    for (key, value) in &config.custom_headers {
        gemini_config = gemini_config.with_header(key.clone(), value.clone());
    }
    Ok(Arc::new(GeminiProvider::new(gemini_config)?))
}
