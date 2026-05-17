pub fn parse_frontmatter_description(text: &str) -> String {
    if !text.starts_with("---") {
        return String::new();
    }

    let lines = text.lines().collect::<Vec<_>>();
    if lines.first().is_none_or(|line| line.trim() != "---") {
        return String::new();
    }

    let Some(end_idx) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (line.trim() == "---").then_some(index))
    else {
        return String::new();
    };

    for line in &lines[1..end_idx] {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("description") {
            return value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
        }
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use crate::parse_frontmatter_description;

    #[test]
    fn frontmatter_description_parser_extracts_description() {
        let description = parse_frontmatter_description(
            "---\nname: writer\ndescription: \"Draft concise text\"\n---\n# Writer",
        );

        assert_eq!(description, "Draft concise text");
    }

    #[test]
    fn frontmatter_description_parser_ignores_missing_or_malformed_blocks() {
        assert_eq!(parse_frontmatter_description("# No frontmatter"), "");
        assert_eq!(
            parse_frontmatter_description("---\nname: writer\n# no closing marker"),
            ""
        );
    }
}
