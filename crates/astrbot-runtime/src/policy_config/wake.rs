use astrbot_pipeline::WakeCheckConfig;
use serde::{Deserialize, Serialize};

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
