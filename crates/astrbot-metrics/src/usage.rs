use astrbot_provider::ProviderTokenUsage;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageRecord {
    pub input_other_tokens: u64,
    pub input_cached_tokens: u64,
    pub output_tokens: u64,
}

impl UsageRecord {
    pub fn new(input_other_tokens: u64, input_cached_tokens: u64, output_tokens: u64) -> Self {
        Self {
            input_other_tokens,
            input_cached_tokens,
            output_tokens,
        }
    }

    pub fn from_provider_usage(usage: ProviderTokenUsage) -> Self {
        Self {
            input_other_tokens: usage.input_other,
            input_cached_tokens: usage.input_cached,
            output_tokens: usage.output,
        }
    }

    pub fn input_tokens(&self) -> u64 {
        self.input_other_tokens + self.input_cached_tokens
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens() + self.output_tokens
    }

    pub fn is_empty(&self) -> bool {
        self.total_tokens() == 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenPrice {
    pub input_per_million_microunits: u64,
    pub cached_input_per_million_microunits: u64,
    pub output_per_million_microunits: u64,
}

impl TokenPrice {
    pub fn new(
        input_per_million_microunits: u64,
        cached_input_per_million_microunits: u64,
        output_per_million_microunits: u64,
    ) -> Self {
        Self {
            input_per_million_microunits,
            cached_input_per_million_microunits,
            output_per_million_microunits,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageCharge {
    pub input_microunits: u64,
    pub cached_input_microunits: u64,
    pub output_microunits: u64,
}

impl UsageCharge {
    pub fn total_microunits(&self) -> u64 {
        self.input_microunits + self.cached_input_microunits + self.output_microunits
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UsageAccountant;

impl UsageAccountant {
    pub fn charge(&self, usage: UsageRecord, price: TokenPrice) -> UsageCharge {
        UsageCharge {
            input_microunits: price_for_tokens(
                usage.input_other_tokens,
                price.input_per_million_microunits,
            ),
            cached_input_microunits: price_for_tokens(
                usage.input_cached_tokens,
                price.cached_input_per_million_microunits,
            ),
            output_microunits: price_for_tokens(
                usage.output_tokens,
                price.output_per_million_microunits,
            ),
        }
    }
}

fn price_for_tokens(tokens: u64, per_million_microunits: u64) -> u64 {
    tokens.saturating_mul(per_million_microunits) / 1_000_000
}

#[cfg(test)]
mod tests {
    use astrbot_provider::ProviderTokenUsage;

    use super::{TokenPrice, UsageAccountant, UsageRecord};

    #[test]
    fn provider_token_usage_maps_to_accounting_without_storage_dependency() {
        let usage = UsageRecord::from_provider_usage(ProviderTokenUsage::openai(100, 40, 20));

        assert_eq!(usage.input_other_tokens, 60);
        assert_eq!(usage.input_cached_tokens, 40);
        assert_eq!(usage.output_tokens, 20);
        assert_eq!(usage.total_tokens(), 120);
    }

    #[test]
    fn usage_accountant_separates_cached_input_and_output_rates() {
        let accountant = UsageAccountant;
        let charge = accountant.charge(
            UsageRecord::new(1_000_000, 2_000_000, 3_000_000),
            TokenPrice::new(10, 2, 30),
        );

        assert_eq!(charge.input_microunits, 10);
        assert_eq!(charge.cached_input_microunits, 4);
        assert_eq!(charge.output_microunits, 90);
        assert_eq!(charge.total_microunits(), 104);
    }
}
