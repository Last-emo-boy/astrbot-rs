use astrbot_pipeline::ResultDecorateConfig;
use serde::{Deserialize, Serialize};

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
