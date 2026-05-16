use std::fmt;

pub const REDACTED_SECRET: &str = "<redacted>";

#[derive(Clone, PartialEq, Eq)]
pub struct SecretValue {
    value: String,
}

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn expose_secret(&self) -> &str {
        &self.value
    }

    pub fn redacted(&self) -> &'static str {
        REDACTED_SECRET
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretValue")
            .field("value", &REDACTED_SECRET)
            .finish()
    }
}

pub fn redact_optional_secret(value: Option<&str>) -> Option<&'static str> {
    value
        .filter(|secret| !secret.is_empty())
        .map(|_| REDACTED_SECRET)
}
