use serde::{Deserialize, Serialize};

use crate::defaults::default_true;

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
