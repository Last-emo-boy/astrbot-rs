use std::sync::Arc;

use astrbot_core::Result;

use crate::config::EmbeddingProviderConfig;
use crate::{
    EmbeddingProvider, GeminiEmbeddingConfig, GeminiEmbeddingProvider, OpenAiEmbeddingConfig,
    OpenAiEmbeddingProvider,
};

pub(crate) fn build_openai_embedding_provider(
    config: &EmbeddingProviderConfig,
) -> Result<Arc<dyn EmbeddingProvider>> {
    let api_base = normalize_openai_embedding_api_base(config.api_base.as_deref());
    let model = config
        .model
        .clone()
        .unwrap_or_else(|| "text-embedding-3-small".to_string());
    let mut embedding_config =
        OpenAiEmbeddingConfig::new(api_base, model).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        embedding_config = embedding_config.with_api_key(api_key.clone());
    }
    if let Some(dimensions) = config.dimensions {
        embedding_config = embedding_config.with_dimensions(dimensions);
    }
    for (key, value) in &config.custom_headers {
        embedding_config = embedding_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(OpenAiEmbeddingProvider::new(embedding_config)?))
}

fn normalize_openai_embedding_api_base(api_base: Option<&str>) -> String {
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

pub(crate) fn build_gemini_embedding_provider(
    config: &EmbeddingProviderConfig,
) -> Result<Arc<dyn EmbeddingProvider>> {
    let api_base = config
        .api_base
        .clone()
        .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string());
    let model = config
        .model
        .clone()
        .unwrap_or_else(|| "gemini-embedding-001".to_string());
    let mut embedding_config =
        GeminiEmbeddingConfig::new(api_base, model).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        embedding_config = embedding_config.with_api_key(api_key.clone());
    }
    if let Some(dimensions) = config.dimensions {
        embedding_config = embedding_config.with_dimensions(dimensions);
    }
    for (key, value) in &config.custom_headers {
        embedding_config = embedding_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(GeminiEmbeddingProvider::new(embedding_config)?))
}
