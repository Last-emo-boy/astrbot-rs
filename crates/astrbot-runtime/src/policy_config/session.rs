use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSessionStatusConfig {
    #[serde(default)]
    pub disabled_sessions: Vec<String>,
}
