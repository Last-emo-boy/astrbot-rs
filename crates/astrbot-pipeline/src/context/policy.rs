use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WakeCheckConfig {
    pub wake_prefixes: Vec<String>,
    pub direct_message_needs_wake_prefix: bool,
    pub ignore_bot_self_message: bool,
    pub ignore_at_all: bool,
    pub bot_self_id: Option<String>,
}

impl WakeCheckConfig {
    pub fn with_wake_prefixes<I, S>(mut self, wake_prefixes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.wake_prefixes = wake_prefixes
            .into_iter()
            .map(Into::into)
            .map(|prefix| prefix.trim().to_string())
            .filter(|prefix| !prefix.is_empty())
            .collect();
        self
    }

    pub fn require_wake_prefix_for_direct_messages(mut self, required: bool) -> Self {
        self.direct_message_needs_wake_prefix = required;
        self
    }

    pub fn ignore_bot_self_message(mut self, ignore: bool) -> Self {
        self.ignore_bot_self_message = ignore;
        self
    }

    pub fn ignore_at_all(mut self, ignore: bool) -> Self {
        self.ignore_at_all = ignore;
        self
    }

    pub fn with_bot_self_id(mut self, bot_self_id: impl Into<String>) -> Self {
        let bot_self_id = bot_self_id.into().trim().to_string();
        self.bot_self_id = (!bot_self_id.is_empty()).then_some(bot_self_id);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WhitelistPolicyConfig {
    pub enabled: bool,
    pub allowed_ids: Vec<String>,
    pub bypass_platform_ids: Vec<String>,
    pub admin_user_ids: Vec<String>,
    pub ignore_admin_on_group: bool,
    pub ignore_admin_on_direct: bool,
    pub log_denies: bool,
}

impl WhitelistPolicyConfig {
    pub fn enabled<I, S>(allowed_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::default().enable().with_allowed_ids(allowed_ids)
    }

    pub fn enable(mut self) -> Self {
        self.enabled = true;
        self
    }

    pub fn with_allowed_ids<I, S>(mut self, allowed_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_ids = normalize_ids(allowed_ids);
        self
    }

    pub fn with_bypass_platform_ids<I, S>(mut self, bypass_platform_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.bypass_platform_ids = normalize_ids(bypass_platform_ids);
        self
    }

    pub fn with_admin_user_ids<I, S>(mut self, admin_user_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.admin_user_ids = normalize_ids(admin_user_ids);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub max_events: usize,
    pub window: Duration,
    pub strategy: RateLimitStrategy,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_events: 0,
            window: Duration::from_secs(0),
            strategy: RateLimitStrategy::Discard,
        }
    }
}

impl RateLimitConfig {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub fn fixed_window(max_events: usize, window: Duration, strategy: RateLimitStrategy) -> Self {
        Self {
            enabled: true,
            max_events,
            window,
            strategy,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateLimitStrategy {
    Stall,
    Discard,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderFallbackConfig {
    pub enabled: bool,
    pub require_wake: bool,
    pub error_message: Option<String>,
    pub provider_wake_prefix: Option<String>,
}

impl Default for ProviderFallbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            require_wake: false,
            error_message: Some(default_provider_error_message()),
            provider_wake_prefix: Some(String::new()),
        }
    }
}

impl ProviderFallbackConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    pub fn require_wake(mut self, require_wake: bool) -> Self {
        self.require_wake = require_wake;
        self
    }

    pub fn with_error_message(mut self, error_message: impl Into<String>) -> Self {
        self.error_message = non_empty_option(error_message);
        self
    }

    pub fn with_provider_wake_prefix(mut self, provider_wake_prefix: impl Into<String>) -> Self {
        self.provider_wake_prefix = Some(provider_wake_prefix.into().trim().to_string());
        self
    }

    pub fn with_provider_wake_prefixes<I, S>(
        mut self,
        provider_wake_prefix: impl Into<String>,
        bot_wake_prefixes: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut provider_wake_prefix = provider_wake_prefix.into().trim().to_string();
        for bot_wake_prefix in bot_wake_prefixes {
            let bot_wake_prefix = bot_wake_prefix.as_ref();
            if !bot_wake_prefix.is_empty() && provider_wake_prefix.starts_with(bot_wake_prefix) {
                provider_wake_prefix = provider_wake_prefix[bot_wake_prefix.len()..].to_string();
                break;
            }
        }
        self.provider_wake_prefix = Some(provider_wake_prefix);
        self
    }

    pub fn without_error_message(mut self) -> Self {
        self.error_message = None;
        self
    }
}

pub(crate) fn normalize_ids<I, S>(ids: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    ids.into_iter()
        .map(Into::into)
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect()
}

fn default_provider_error_message() -> String {
    "LLM 请求失败，请稍后再试。".to_string()
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then_some(value)
}
