use astrbot_pipeline::WhitelistPolicyConfig;
use serde::{Deserialize, Serialize};

use crate::defaults::default_whitelist_bypass_platform_ids;

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
