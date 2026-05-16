pub(crate) fn sanitize_tool_name_fragment(name: &str) -> String {
    let mut sanitized = String::new();
    let mut last_was_separator = false;

    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            sanitized.push(character.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator && !sanitized.is_empty() {
            sanitized.push('_');
            last_was_separator = true;
        }
    }

    while sanitized.ends_with('_') {
        sanitized.pop();
    }

    if sanitized.is_empty() {
        "server".to_string()
    } else {
        sanitized
    }
}
