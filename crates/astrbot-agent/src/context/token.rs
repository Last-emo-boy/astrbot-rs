use astrbot_core::{ProviderContentPart, ProviderContextMessage};

pub trait AgentTokenCounter: Send + Sync {
    fn count_text(&self, text: &str) -> usize;

    fn count_part(&self, part: &ProviderContentPart) -> usize {
        match part {
            ProviderContentPart::Text { text } => self.count_text(text),
            ProviderContentPart::ImageUrl { url } => self.count_text(url).max(1),
        }
    }

    fn count_message(&self, message: &ProviderContextMessage) -> usize {
        message.parts.iter().map(|part| self.count_part(part)).sum()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ApproximateTokenCounter;

impl AgentTokenCounter for ApproximateTokenCounter {
    fn count_text(&self, text: &str) -> usize {
        let mut chinese_chars = 0usize;
        let mut other_chars = 0usize;

        for character in text.chars() {
            if character.is_whitespace() {
                continue;
            }

            if ('\u{4e00}'..='\u{9fff}').contains(&character) {
                chinese_chars += 1;
            } else {
                other_chars += 1;
            }
        }

        let weighted_tenths = chinese_chars * 6 + other_chars * 3;
        weighted_tenths.div_ceil(10)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextTokenBudget {
    max_context_tokens: usize,
    reserved_response_tokens: usize,
}

impl ContextTokenBudget {
    pub fn unlimited() -> Self {
        Self {
            max_context_tokens: 0,
            reserved_response_tokens: 0,
        }
    }

    pub fn new(max_context_tokens: usize) -> Self {
        Self {
            max_context_tokens,
            reserved_response_tokens: 0,
        }
    }

    pub fn with_reserved_response_tokens(mut self, reserved_response_tokens: usize) -> Self {
        self.reserved_response_tokens = reserved_response_tokens;
        self
    }

    pub fn max_context_tokens(&self) -> usize {
        self.max_context_tokens
    }

    pub fn reserved_response_tokens(&self) -> usize {
        self.reserved_response_tokens
    }

    pub fn available_context_tokens(&self) -> Option<usize> {
        (self.max_context_tokens > 0).then_some(
            self.max_context_tokens
                .saturating_sub(self.reserved_response_tokens),
        )
    }
}

impl Default for ContextTokenBudget {
    fn default() -> Self {
        Self::unlimited()
    }
}
