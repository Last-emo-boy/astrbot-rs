use serde_json::Value;

const ERROR_TEXT_MAX_CHARS: usize = 4096;

pub(crate) fn extract_error_message(body: &str) -> String {
    let fallback = truncate(body.trim());
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return fallback;
    };

    let extracted = value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .or_else(|| value.get("detail").and_then(Value::as_str))
        .or_else(|| value.get("message").and_then(Value::as_str))
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_string);

    extracted.unwrap_or(fallback)
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= ERROR_TEXT_MAX_CHARS {
        return text.to_string();
    }

    text.chars().take(ERROR_TEXT_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::extract_error_message;

    #[test]
    fn extracts_nested_error_message() {
        assert_eq!(
            extract_error_message(r#"{"error":{"message":"bad request"}}"#),
            "bad request"
        );
    }

    #[test]
    fn falls_back_to_trimmed_body() {
        assert_eq!(extract_error_message(" plain error "), "plain error");
    }
}
