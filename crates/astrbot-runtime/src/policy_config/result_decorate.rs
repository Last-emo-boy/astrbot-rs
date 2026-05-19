use astrbot_pipeline::{
    ResultDecorateConfig, TextToImageDecorateConfig, TextToSpeechDecorateConfig,
};
use astrbot_render::{RenderMode, RenderStrategy, TemplateName};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeResultDecorateConfig {
    #[serde(default)]
    pub reply_prefix: Option<String>,
    #[serde(default)]
    pub only_llm_result: bool,
    #[serde(default)]
    pub t2i_enabled: bool,
    #[serde(default = "default_t2i_word_threshold")]
    pub t2i_word_threshold: usize,
    #[serde(default = "default_t2i_strategy")]
    pub t2i_strategy: String,
    #[serde(default)]
    pub t2i_endpoint: Option<String>,
    #[serde(default)]
    pub t2i_use_file_service: bool,
    #[serde(default = "default_t2i_active_template")]
    pub t2i_active_template: String,
    #[serde(default)]
    pub tts_enabled: bool,
    #[serde(default)]
    pub tts_provider_id: Option<String>,
    #[serde(default)]
    pub tts_dual_output: bool,
    #[serde(default)]
    pub tts_use_file_service: bool,
    #[serde(default)]
    pub content_safety_after_transform: bool,
}

impl Default for RuntimeResultDecorateConfig {
    fn default() -> Self {
        Self {
            reply_prefix: None,
            only_llm_result: false,
            t2i_enabled: false,
            t2i_word_threshold: default_t2i_word_threshold(),
            t2i_strategy: default_t2i_strategy(),
            t2i_endpoint: None,
            t2i_use_file_service: false,
            t2i_active_template: default_t2i_active_template(),
            tts_enabled: false,
            tts_provider_id: None,
            tts_dual_output: false,
            tts_use_file_service: false,
            content_safety_after_transform: false,
        }
    }
}

impl From<RuntimeResultDecorateConfig> for ResultDecorateConfig {
    fn from(config: RuntimeResultDecorateConfig) -> Self {
        let mut result_decorate = ResultDecorateConfig::default();
        if let Some(reply_prefix) = config.reply_prefix {
            result_decorate = result_decorate.with_reply_prefix(reply_prefix);
        }
        let mut tts = TextToSpeechDecorateConfig::default();
        if config.tts_enabled {
            tts = TextToSpeechDecorateConfig::enabled()
                .with_dual_output(config.tts_dual_output)
                .with_file_service(config.tts_use_file_service);
            if let Some(provider_id) = config.tts_provider_id {
                tts = tts.with_provider_id(provider_id);
            }
        }

        let active_template =
            TemplateName::new(config.t2i_active_template).unwrap_or_else(|_| TemplateName::base());
        let strategy = match config.t2i_strategy.trim() {
            "remote" | "network" | "network_only" => RenderStrategy::NetworkOnly,
            "local" | "local_only" => RenderStrategy::LocalOnly,
            _ => RenderStrategy::NetworkPreferred,
        };
        let mode =
            if matches!(strategy, RenderStrategy::NetworkOnly) && !config.t2i_use_file_service {
                RenderMode::Url
            } else {
                RenderMode::File
            };
        let mut t2i = TextToImageDecorateConfig::default()
            .with_word_threshold(config.t2i_word_threshold)
            .with_strategy(strategy)
            .with_mode(mode)
            .with_active_template(active_template)
            .with_file_service(config.t2i_use_file_service);
        t2i.enabled = config.t2i_enabled;

        result_decorate
            .only_llm_result(config.only_llm_result)
            .with_tts(tts)
            .with_t2i(t2i)
            .with_content_safety_after_transform(config.content_safety_after_transform)
    }
}

fn default_t2i_word_threshold() -> usize {
    150
}

fn default_t2i_strategy() -> String {
    "local".to_string()
}

fn default_t2i_active_template() -> String {
    "base".to_string()
}
