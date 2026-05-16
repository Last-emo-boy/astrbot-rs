use std::collections::HashMap;

use astrbot_core::{AstrbotError, Result};

use crate::factories::common::{parse_provider_bool, parse_provider_f32, provider_option};

pub(super) fn option<'a>(options: &'a HashMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    provider_option(options, keys)
}

pub(super) fn bool_option(value: &str, option_name: &str) -> Result<bool> {
    parse_provider_bool(value, option_name)
}

pub(super) fn f32_option(value: &str, option_name: &str) -> Result<f32> {
    parse_provider_f32(value, option_name)
}

pub(super) fn json_option(value: &str, context: &str) -> Result<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(value)
        .map_err(|err| AstrbotError::Provider(format!("{context}: {err}")))
}

pub(super) fn named_f32_option(value: &str, error_prefix: &str) -> Result<f32> {
    value
        .parse::<f32>()
        .map_err(|_| AstrbotError::Provider(format!("{error_prefix}: {value}")))
}
