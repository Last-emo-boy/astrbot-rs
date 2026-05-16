use std::sync::Arc;
use std::time::Duration;

use astrbot_pipeline::{
    ContentSafetyConfig, KeywordContentSafetyStrategy, ProviderFallbackConfig, RateLimitConfig,
    RateLimitStrategy, ResultDecorateConfig, WakeCheckConfig, WhitelistPolicyConfig,
};
use serde::{Deserialize, Serialize};

use crate::defaults::{
    default_provider_error_message_option, default_true, default_whitelist_bypass_platform_ids,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeWakeCheckConfig {
    #[serde(default)]
    pub wake_prefixes: Vec<String>,
    #[serde(default)]
    pub direct_message_needs_wake_prefix: bool,
    #[serde(default)]
    pub ignore_bot_self_message: bool,
    #[serde(default)]
    pub ignore_at_all: bool,
    #[serde(default)]
    pub bot_self_id: Option<String>,
}

impl From<RuntimeWakeCheckConfig> for WakeCheckConfig {
    fn from(config: RuntimeWakeCheckConfig) -> Self {
        let mut wake_check = WakeCheckConfig::default().with_wake_prefixes(config.wake_prefixes);
        wake_check.direct_message_needs_wake_prefix = config.direct_message_needs_wake_prefix;
        wake_check.ignore_bot_self_message = config.ignore_bot_self_message;
        wake_check.ignore_at_all = config.ignore_at_all;
        wake_check.bot_self_id = config
            .bot_self_id
            .map(|bot_self_id| bot_self_id.trim().to_string())
            .and_then(|bot_self_id| (!bot_self_id.is_empty()).then_some(bot_self_id));
        wake_check
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeWhitelistPolicyConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub allowed_ids: Vec<String>,
    #[serde(default = "default_whitelist_bypass_platform_ids")]
    pub bypass_platform_ids: Vec<String>,
    #[serde(default)]
    pub admin_user_ids: Vec<String>,
    #[serde(default)]
    pub ignore_admin_on_group: bool,
    #[serde(default)]
    pub ignore_admin_on_direct: bool,
    #[serde(default)]
    pub log_denies: bool,
}

impl Default for RuntimeWhitelistPolicyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_ids: Vec::new(),
            bypass_platform_ids: default_whitelist_bypass_platform_ids(),
            admin_user_ids: Vec::new(),
            ignore_admin_on_group: false,
            ignore_admin_on_direct: false,
            log_denies: false,
        }
    }
}

impl From<RuntimeWhitelistPolicyConfig> for WhitelistPolicyConfig {
    fn from(config: RuntimeWhitelistPolicyConfig) -> Self {
        let mut policy = WhitelistPolicyConfig::default()
            .with_allowed_ids(config.allowed_ids)
            .with_bypass_platform_ids(config.bypass_platform_ids)
            .with_admin_user_ids(config.admin_user_ids);
        policy.enabled = config.enabled;
        policy.ignore_admin_on_group = config.ignore_admin_on_group;
        policy.ignore_admin_on_direct = config.ignore_admin_on_direct;
        policy.log_denies = config.log_denies;
        policy
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSessionStatusConfig {
    #[serde(default)]
    pub disabled_sessions: Vec<String>,
}

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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeResultDecorateConfig {
    #[serde(default)]
    pub reply_prefix: Option<String>,
    #[serde(default)]
    pub only_llm_result: bool,
}

impl From<RuntimeResultDecorateConfig> for ResultDecorateConfig {
    fn from(config: RuntimeResultDecorateConfig) -> Self {
        let mut result_decorate = ResultDecorateConfig::default();
        if let Some(reply_prefix) = config.reply_prefix {
            result_decorate = result_decorate.with_reply_prefix(reply_prefix);
        }
        result_decorate.only_llm_result(config.only_llm_result)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStatePolicyConfig {
    #[serde(default = "default_true")]
    pub preserve_provider_preference_on_restart: bool,
}

impl Default for RuntimeStatePolicyConfig {
    fn default() -> Self {
        Self {
            preserve_provider_preference_on_restart: true,
        }
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
