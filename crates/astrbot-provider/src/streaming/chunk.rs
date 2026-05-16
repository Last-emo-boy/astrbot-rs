use serde_json::Value;

pub(crate) fn normalize_stream_text_delta(value: &Value) -> String {
    match value {
        Value::String(text) => text.to_string(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                if item.get("type").and_then(Value::as_str) == Some("text") {
                    item.get("text").and_then(Value::as_str)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(""),
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::normalize_stream_text_delta;

    #[test]
    fn normalizes_stream_text_delta_shapes() {
        assert_eq!(normalize_stream_text_delta(&json!("hello")), "hello");
        assert_eq!(
            normalize_stream_text_delta(&json!([
                {"type": "text", "text": "hello"},
                {"type": "text", "text": " world"},
                {"type": "image_url", "image_url": {"url": "ignored"}}
            ])),
            "hello world"
        );
        assert_eq!(
            normalize_stream_text_delta(&json!({"type": "text", "text": "hello"})),
            "hello"
        );
    }
}
