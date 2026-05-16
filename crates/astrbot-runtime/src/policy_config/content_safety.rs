use std::sync::Arc;

use astrbot_pipeline::{ContentSafetyConfig, KeywordContentSafetyStrategy};
use serde::{Deserialize, Serialize};

use crate::defaults::default_true;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeContentSafetyConfig {
    #[serde(default)]
    pub rejection_message: Option<String>,
    #[serde(default)]
    pub internal_keywords: RuntimeKeywordContentSafetyConfig,
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
