pub(crate) fn sse_data_lines(body: &str) -> impl Iterator<Item = &str> {
    body.split("\n\n")
        .flat_map(str::lines)
        .filter_map(|line| line.trim().strip_prefix("data:").map(str::trim))
        .filter(|data| !data.is_empty())
}

#[cfg(test)]
mod tests {
    use super::sse_data_lines;

    #[test]
    fn extracts_sse_data_lines() {
        let lines = sse_data_lines("event: message\ndata: one\n\ndata: two\n\n: keepalive\n")
            .collect::<Vec<_>>();

        assert_eq!(lines, vec!["one", "two"]);
    }
}
