#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResultDecorateConfig {
    pub reply_prefix: Option<String>,
    pub only_llm_result: bool,
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
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then_some(value)
}
