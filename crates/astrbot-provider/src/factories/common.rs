use std::collections::HashMap;

use astrbot_core::{AstrbotError, Result};

pub(super) fn provider_option<'a>(
    options: &'a HashMap<String, String>,
    keys: &[&str],
) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        options
            .get(*key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn parse_provider_f32(value: &str, option_name: &str) -> Result<f32> {
    value.parse::<f32>().map_err(|_| {
        AstrbotError::Provider(format!(
            "invalid numeric provider option {option_name}: {value}"
        ))
    })
}

pub(super) fn parse_provider_bool(value: &str, option_name: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(AstrbotError::Provider(format!(
            "invalid boolean provider option {option_name}: {value}"
        ))),
    }
}
