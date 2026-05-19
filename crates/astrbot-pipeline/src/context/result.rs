use astrbot_render::{RenderMode, RenderStrategy, TemplateName};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResultDecorateConfig {
    pub reply_prefix: Option<String>,
    pub only_llm_result: bool,
    pub tts: TextToSpeechDecorateConfig,
    pub t2i: TextToImageDecorateConfig,
    pub content_safety_after_transform: bool,
}

impl ResultDecorateConfig {
    pub fn with_reply_prefix(mut self, reply_prefix: impl Into<String>) -> Self {
        self.reply_prefix = non_empty_option(reply_prefix);
        self
    }

    pub fn only_llm_result(mut self, only_llm_result: bool) -> Self {
        self.only_llm_result = only_llm_result;
        self
    }

    pub fn with_tts(mut self, tts: TextToSpeechDecorateConfig) -> Self {
        self.tts = tts;
        self
    }

    pub fn with_t2i(mut self, t2i: TextToImageDecorateConfig) -> Self {
        self.t2i = t2i;
        self
    }

    pub fn with_content_safety_after_transform(mut self, enabled: bool) -> Self {
        self.content_safety_after_transform = enabled;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextToSpeechDecorateConfig {
    pub enabled: bool,
    pub provider_id: Option<String>,
    pub dual_output: bool,
    pub use_file_service: bool,
}

impl TextToSpeechDecorateConfig {
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = non_empty_option(provider_id);
        self
    }

    pub fn with_dual_output(mut self, dual_output: bool) -> Self {
        self.dual_output = dual_output;
        self
    }

    pub fn with_file_service(mut self, use_file_service: bool) -> Self {
        self.use_file_service = use_file_service;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextToImageDecorateConfig {
    pub enabled: bool,
    pub word_threshold: usize,
    pub strategy: RenderStrategy,
    pub mode: RenderMode,
    pub active_template: TemplateName,
    pub use_file_service: bool,
}

impl Default for TextToImageDecorateConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            word_threshold: 150,
            strategy: RenderStrategy::NetworkPreferred,
            mode: RenderMode::File,
            active_template: TemplateName::base(),
            use_file_service: false,
        }
    }
}

impl TextToImageDecorateConfig {
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    pub fn with_word_threshold(mut self, word_threshold: usize) -> Self {
        self.word_threshold = word_threshold.max(50);
        self
    }

    pub fn with_strategy(mut self, strategy: RenderStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn with_mode(mut self, mode: RenderMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_active_template(mut self, active_template: TemplateName) -> Self {
        self.active_template = active_template;
        self
    }

    pub fn with_file_service(mut self, use_file_service: bool) -> Self {
        self.use_file_service = use_file_service;
        self
    }
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then_some(value)
}
