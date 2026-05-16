pub(crate) fn join_api_path(api_base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        api_base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::join_api_path;

    #[test]
    fn joins_base_and_path_without_double_slash() {
        assert_eq!(
            join_api_path("https://api.example/v1/", "/chat/completions"),
            "https://api.example/v1/chat/completions"
        );
    }
}
