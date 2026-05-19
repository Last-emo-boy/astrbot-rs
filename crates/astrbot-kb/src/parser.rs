use astrbot_core::Result;
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaItem {
    pub media_type: String,
    pub file_name: String,
    pub content: Vec<u8>,
    pub mime_type: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParseResult {
    pub text: String,
    pub media: Vec<MediaItem>,
}

#[async_trait]
pub trait DocumentParser: Send + Sync {
    async fn parse(&self, file_content: Vec<u8>, file_name: &str) -> Result<ParseResult>;
}

#[derive(Clone, Debug, Default)]
pub struct PlainTextParser;

#[async_trait]
impl DocumentParser for PlainTextParser {
    async fn parse(&self, file_content: Vec<u8>, _file_name: &str) -> Result<ParseResult> {
        let text = String::from_utf8(file_content)
            .map_err(|error| crate::types::kb_error(format!("invalid utf-8 text: {error}")))?;
        Ok(ParseResult {
            text,
            media: Vec::new(),
        })
    }
}

/// Markdown → plain text. Strips YAML/TOML front-matter, removes most
/// Markdown decoration so embeddings see prose instead of syntax.
#[derive(Clone, Debug, Default)]
pub struct MarkdownParser;

#[async_trait]
impl DocumentParser for MarkdownParser {
    async fn parse(&self, file_content: Vec<u8>, _file_name: &str) -> Result<ParseResult> {
        let source = String::from_utf8(file_content)
            .map_err(|error| crate::types::kb_error(format!("invalid utf-8 markdown: {error}")))?;
        let text = strip_markdown(&source);
        Ok(ParseResult {
            text,
            media: Vec::new(),
        })
    }
}

/// HTML → plain text. Strips tags + collapses whitespace.
#[derive(Clone, Debug, Default)]
pub struct HtmlTextParser;

#[async_trait]
impl DocumentParser for HtmlTextParser {
    async fn parse(&self, file_content: Vec<u8>, _file_name: &str) -> Result<ParseResult> {
        let source = String::from_utf8(file_content)
            .map_err(|error| crate::types::kb_error(format!("invalid utf-8 html: {error}")))?;
        let text = strip_html(&source);
        Ok(ParseResult {
            text,
            media: Vec::new(),
        })
    }
}

pub fn strip_markdown(source: &str) -> String {
    let body = drop_front_matter(source);
    let mut out = String::with_capacity(body.len());
    let mut in_code_block = false;
    for raw_line in body.lines() {
        let line = raw_line.trim_end();
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            out.push('\n');
            continue;
        }
        if in_code_block {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let stripped = strip_markdown_line(line);
        if !stripped.is_empty() {
            out.push_str(&stripped);
        }
        out.push('\n');
    }
    out.trim().to_string()
}

fn drop_front_matter(source: &str) -> &str {
    if source.starts_with("---\n") {
        if let Some(end) = source[4..].find("\n---\n") {
            return &source[4 + end + 5..];
        }
        if let Some(end) = source[4..].find("\n---") {
            return &source[4 + end + 4..];
        }
    }
    if source.starts_with("+++\n") {
        if let Some(end) = source[4..].find("\n+++\n") {
            return &source[4 + end + 5..];
        }
    }
    source
}

fn strip_markdown_line(line: &str) -> String {
    let mut s = line.trim_start();
    let mut hash_count = 0;
    while s.starts_with('#') && hash_count < 6 {
        hash_count += 1;
        s = &s[1..];
    }
    if hash_count > 0 {
        s = s.trim_start();
    }
    while s.starts_with('>') {
        s = s[1..].trim_start();
    }
    if let Some(rest) = s
        .strip_prefix("- ")
        .or_else(|| s.strip_prefix("* "))
        .or_else(|| s.strip_prefix("+ "))
    {
        s = rest;
    }
    if let Some(rest) = strip_ordered_list_prefix(s) {
        s = rest;
    }
    strip_inline_markdown(s)
}

fn strip_ordered_list_prefix(input: &str) -> Option<&str> {
    let digits: String = input.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let rest = &input[digits.len()..];
    let rest = rest.strip_prefix('.')?;
    Some(rest.trim_start())
}

fn strip_inline_markdown(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'`' => {
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != b'`' {
                    j += 1;
                }
                let end = j.min(bytes.len());
                out.push_str(&input[i + 1..end]);
                i = j + 1;
                continue;
            }
            b'!' if bytes.get(i + 1) == Some(&b'[') => {
                if let Some(end) = find_unescaped(input, i + 2, b']') {
                    if bytes.get(end + 1) == Some(&b'(') {
                        if let Some(close) = find_unescaped(input, end + 2, b')') {
                            i = close + 1;
                            continue;
                        }
                    }
                    out.push_str(&input[i + 2..end]);
                    i = end + 1;
                    continue;
                }
                out.push(b as char);
                i += 1;
            }
            b'[' => {
                if let Some(end) = find_unescaped(input, i + 1, b']') {
                    if bytes.get(end + 1) == Some(&b'(') {
                        if let Some(close) = find_unescaped(input, end + 2, b')') {
                            out.push_str(&input[i + 1..end]);
                            i = close + 1;
                            continue;
                        }
                    }
                    out.push_str(&input[i + 1..end]);
                    i = end + 1;
                    continue;
                }
                out.push(b as char);
                i += 1;
            }
            b'*' | b'_' => {
                if bytes.get(i + 1) == Some(&b) {
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }
            b'~' => {
                if bytes.get(i + 1) == Some(&b'~') {
                    i += 2;
                    continue;
                }
                out.push(b as char);
                i += 1;
                continue;
            }
            _ => {
                if b < 0x80 {
                    out.push(b as char);
                    i += 1;
                } else {
                    let next = next_char_boundary(input, i);
                    out.push_str(&input[i..next]);
                    i = next;
                }
            }
        }
    }
    out.trim().to_string()
}

fn find_unescaped(input: &str, from: usize, target: u8) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == target {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn next_char_boundary(input: &str, mut i: usize) -> usize {
    let len = input.len();
    if i >= len {
        return len;
    }
    i += 1;
    while i < len && !input.is_char_boundary(i) {
        i += 1;
    }
    i
}

pub fn strip_html(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_tag = false;
    let mut in_script_or_style = false;
    let mut buffer = String::new();
    for c in source.chars() {
        if in_tag {
            buffer.push(c);
            if c == '>' {
                let tag = buffer.trim_end_matches('>').trim().to_ascii_lowercase();
                if tag.starts_with("script") || tag.starts_with("style") {
                    in_script_or_style = true;
                } else if tag.starts_with("/script") || tag.starts_with("/style") {
                    in_script_or_style = false;
                }
                buffer.clear();
                in_tag = false;
            }
            continue;
        }
        if c == '<' {
            in_tag = true;
            continue;
        }
        if !in_script_or_style {
            out.push(c);
        }
    }
    decode_html_entities(&collapse_whitespace(out))
}

fn collapse_whitespace(input: String) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_space = false;
    for c in input.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(c);
            last_was_space = false;
        }
    }
    out.trim().to_string()
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn markdown_strips_headings_and_emphasis() {
        let parser = MarkdownParser;
        let source = b"# Title\n\nSome **bold** and *italic* text.";
        let result = parser.parse(source.to_vec(), "doc.md").await.unwrap();
        assert!(result.text.contains("Title"));
        assert!(result.text.contains("Some bold and italic text"));
        assert!(!result.text.contains("**"));
    }

    #[tokio::test]
    async fn markdown_drops_yaml_front_matter() {
        let parser = MarkdownParser;
        let source = b"---\ntitle: x\n---\n\nBody only.";
        let result = parser.parse(source.to_vec(), "doc.md").await.unwrap();
        assert_eq!(result.text, "Body only.");
    }

    #[tokio::test]
    async fn markdown_extracts_link_label_drops_target() {
        let parser = MarkdownParser;
        let source = b"See [docs](https://example.com/x) for more.";
        let result = parser.parse(source.to_vec(), "doc.md").await.unwrap();
        assert!(result.text.contains("See docs for more"));
        assert!(!result.text.contains("https://"));
    }

    #[tokio::test]
    async fn markdown_keeps_code_block_contents() {
        let parser = MarkdownParser;
        let source = b"## Snippet\n\n```rs\nlet x = 1;\n```\n";
        let result = parser.parse(source.to_vec(), "doc.md").await.unwrap();
        assert!(result.text.contains("Snippet"));
        assert!(result.text.contains("let x = 1;"));
    }

    #[tokio::test]
    async fn html_strips_tags_and_scripts() {
        let parser = HtmlTextParser;
        let source =
            b"<html><body><script>alert('x')</script><p>Hello &amp; goodbye</p></body></html>";
        let result = parser.parse(source.to_vec(), "page.html").await.unwrap();
        assert_eq!(result.text, "Hello & goodbye");
    }

    #[tokio::test]
    async fn html_decodes_common_entities() {
        let parser = HtmlTextParser;
        let source = b"<p>1 &lt; 2 &amp;&amp; 2 &gt; 1</p>";
        let result = parser.parse(source.to_vec(), "page.html").await.unwrap();
        assert_eq!(result.text, "1 < 2 && 2 > 1");
    }

    #[test]
    fn strip_markdown_handles_lists_and_quotes() {
        let source = "> Quote line\n- item one\n* item two\n1. item three";
        let stripped = strip_markdown(source);
        for fragment in ["Quote line", "item one", "item two", "item three"] {
            assert!(stripped.contains(fragment), "missing {fragment}");
        }
    }
}
