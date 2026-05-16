use std::sync::Arc;

use astrbot_core::Result;

use crate::config::RerankProviderConfig;
use crate::{
    BailianRerankConfig, BailianRerankProvider, RerankProvider, VllmRerankConfig,
    VllmRerankProvider, XinferenceRerankConfig, XinferenceRerankProvider,
};

pub(crate) fn build_vllm_rerank_provider(
    config: &RerankProviderConfig,
) -> Result<Arc<dyn RerankProvider>> {
    let api_base = config
        .api_base
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1:8000".to_string());
    let model = config
        .model
        .clone()
        .unwrap_or_else(|| "BAAI/bge-reranker-base".to_string());
    let mut rerank_config = VllmRerankConfig::new(api_base, model).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        rerank_config = rerank_config.with_api_key(api_key.clone());
    }
    for (key, value) in &config.custom_headers {
        rerank_config = rerank_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(VllmRerankProvider::new(rerank_config)?))
}

pub(crate) fn build_bailian_rerank_provider(
    config: &RerankProviderConfig,
) -> Result<Arc<dyn RerankProvider>> {
    let api_base = config.api_base.clone().unwrap_or_else(|| {
        "https://dashscope.aliyuncs.com/api/v1/services/rerank/text-rerank/text-rerank".to_string()
    });
    let model = config
        .model
        .clone()
        .unwrap_or_else(|| "qwen3-rerank".to_string());
    let mut rerank_config = BailianRerankConfig::new(api_base, model).with_timeout(config.timeout);
    if let Some(api_key) = &config.api_key {
        rerank_config = rerank_config.with_api_key(api_key.clone());
    }
    for (key, value) in &config.custom_headers {
        rerank_config = rerank_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(BailianRerankProvider::new(rerank_config)?))
}

pub(crate) fn build_xinference_rerank_provider(
    config: &RerankProviderConfig,
) -> Result<Arc<dyn RerankProvider>> {
    let api_base = config
        .api_base
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1:8000".to_string());
    let model = config
        .model
        .clone()
        .unwrap_or_else(|| "BAAI/bge-reranker-base".to_string());
    let mut rerank_config = XinferenceRerankConfig::new(api_base, model)
        .with_timeout(config.timeout)
        .with_launch_model_if_not_running(config.launch_model_if_not_running);
    if let Some(api_key) = &config.api_key {
        rerank_config = rerank_config.with_api_key(api_key.clone());
    }
    for (key, value) in &config.custom_headers {
        rerank_config = rerank_config.with_header(key.clone(), value.clone());
    }

    Ok(Arc::new(XinferenceRerankProvider::new(rerank_config)?))
}
