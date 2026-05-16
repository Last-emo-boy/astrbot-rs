use std::time::Duration;

use astrbot_pipeline::{RateLimitConfig, RateLimitStrategy};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRateLimitConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub window_seconds: u64,
    #[serde(default)]
    pub strategy: RuntimeRateLimitStrategy,
}

impl Default for RuntimeRateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            count: 0,
            window_seconds: 0,
            strategy: RuntimeRateLimitStrategy::Discard,
        }
    }
}

impl From<RuntimeRateLimitConfig> for RateLimitConfig {
    fn from(config: RuntimeRateLimitConfig) -> Self {
        if !config.enabled {
            return RateLimitConfig::disabled();
        }

        RateLimitConfig::fixed_window(
            config.count,
            Duration::from_secs(config.window_seconds),
            RateLimitStrategy::from(config.strategy),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRateLimitStrategy {
    Stall,
    #[default]
    Discard,
}

impl From<RuntimeRateLimitStrategy> for RateLimitStrategy {
    fn from(strategy: RuntimeRateLimitStrategy) -> Self {
        match strategy {
            RuntimeRateLimitStrategy::Stall => Self::Stall,
            RuntimeRateLimitStrategy::Discard => Self::Discard,
        }
    }
}
