use astrbot_pipeline::ProviderFallbackConfig;
use serde::{Deserialize, Serialize};

use crate::defaults::{default_provider_error_message_option, default_true};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeProviderFallbackConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub require_wake: bool,
    #[serde(default = "default_provider_error_message_option")]
    pub error_message: Option<String>,
}

impl Default for RuntimeProviderFallbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            require_wake: false,
            error_message: default_provider_error_message_option(),
        }
    }
}

impl From<RuntimeProviderFallbackConfig> for ProviderFallbackConfig {
    fn from(config: RuntimeProviderFallbackConfig) -> Self {
        let mut provider_fallback = if config.enabled {
            ProviderFallbackConfig::default()
        } else {
            ProviderFallbackConfig::disabled()
        };
        provider_fallback.require_wake = config.require_wake;
        provider_fallback.error_message = config.error_message;
        provider_fallback
    }
}
