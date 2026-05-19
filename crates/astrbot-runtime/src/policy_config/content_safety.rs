use std::sync::Arc;

use astrbot_pipeline::{
    BaiduAipContentSafetyStrategy, ContentSafetyConfig, KeywordContentSafetyStrategy,
};
use serde::{Deserialize, Serialize};

use crate::defaults::default_true;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeContentSafetyConfig {
    #[serde(default)]
    pub rejection_message: Option<String>,
    #[serde(default)]
    pub internal_keywords: RuntimeKeywordContentSafetyConfig,
    #[serde(default)]
    pub baidu_aip: RuntimeBaiduAipContentSafetyConfig,
}

impl From<RuntimeContentSafetyConfig> for ContentSafetyConfig {
    fn from(config: RuntimeContentSafetyConfig) -> Self {
        let mut content_safety = ContentSafetyConfig::default();
        if let Some(rejection_message) = config.rejection_message {
            content_safety = content_safety.with_rejection_message(rejection_message);
        }

        if config.internal_keywords.enabled {
            let strategy =
                KeywordContentSafetyStrategy::new(config.internal_keywords.extra_keywords);
            if !strategy.is_empty() {
                content_safety = content_safety.with_strategy(Arc::new(strategy));
            }
        }
        if let Some(strategy) = config.baidu_aip.into_strategy() {
            content_safety = content_safety.with_strategy(Arc::new(strategy));
        }

        content_safety
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeKeywordContentSafetyConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub extra_keywords: Vec<String>,
}

impl Default for RuntimeKeywordContentSafetyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            extra_keywords: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBaiduAipContentSafetyConfig {
    #[serde(default, alias = "enable")]
    pub enabled: bool,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub secret_key: String,
    #[serde(default)]
    pub token_url: Option<String>,
    #[serde(default)]
    pub censor_url: Option<String>,
}

impl RuntimeBaiduAipContentSafetyConfig {
    fn into_strategy(self) -> Option<BaiduAipContentSafetyStrategy> {
        if !self.enabled
            || self.app_id.trim().is_empty()
            || self.api_key.trim().is_empty()
            || self.secret_key.trim().is_empty()
        {
            return None;
        }

        let mut strategy =
            BaiduAipContentSafetyStrategy::new(self.app_id, self.api_key, self.secret_key);
        if let (Some(token_url), Some(censor_url)) = (
            non_empty_string(self.token_url),
            non_empty_string(self.censor_url),
        ) {
            strategy = strategy.with_endpoints(token_url, censor_url);
        }
        Some(strategy)
    }
}

fn non_empty_string(value: Option<String>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
