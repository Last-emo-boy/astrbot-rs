use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTokenUsage {
    pub input_other: u64,
    pub input_cached: u64,
    pub output: u64,
}

impl ProviderTokenUsage {
    pub fn new(input_other: u64, input_cached: u64, output: u64) -> Self {
        Self {
            input_other,
            input_cached,
            output,
        }
    }

    pub fn openai(prompt_tokens: u64, cached_tokens: u64, completion_tokens: u64) -> Self {
        Self {
            input_other: prompt_tokens.saturating_sub(cached_tokens),
            input_cached: cached_tokens,
            output: completion_tokens,
        }
    }

    pub fn input(&self) -> u64 {
        self.input_other + self.input_cached
    }

    pub fn total(&self) -> u64 {
        self.input() + self.output
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}
