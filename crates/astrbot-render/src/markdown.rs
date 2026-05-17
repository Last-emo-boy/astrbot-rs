use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownDocument {
    pub blocks: Vec<MarkdownBlock>,
}

impl MarkdownDocument {
    pub fn parse(text: &str) -> Self {
        Self {
            blocks: parse_blocks(text),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkdownBlock {
    Paragraph(Vec<InlineSpan>),
    Heading {
        level: u8,
        text: String,
    },
    Quote(Vec<InlineSpan>),
    ListItem(Vec<InlineSpan>),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    Image {
        alt: String,
        url: String,
    },
    Blank,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InlineSpan {
    Text(String),
    Bold(String),
    Italic(String),
    Strike(String),
    Code(String),
    Underline(String),
}

fn parse_blocks(text: &str) -> Vec<MarkdownBlock> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut blocks = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index].trim_end();
        if line.trim().is_empty() {
            blocks.push(MarkdownBlock::Blank);
            index += 1;
            continue;
        }

        if let Some(language) = line.strip_prefix("```") {
            let language = non_empty(language.trim());
            index += 1;
            let mut code_lines = Vec::new();
            while index < lines.len() && !lines[index].trim_start().starts_with("```") {
                code_lines.push(lines[index]);
                index += 1;
            }
            if index < lines.len() {
                index += 1;
            }
            blocks.push(MarkdownBlock::CodeBlock {
                language,
                code: code_lines.join("\n"),
            });
            continue;
        }

        if let Some((alt, url)) = parse_image(line) {
            blocks.push(MarkdownBlock::Image { alt, url });
            index += 1;
            continue;
        }

        if let Some((level, heading)) = parse_heading(line) {
            blocks.push(MarkdownBlock::Heading {
                level,
                text: heading,
            });
            index += 1;
            continue;
        }

        if let Some(quote) = line.trim_start().strip_prefix('>') {
            blocks.push(MarkdownBlock::Quote(parse_inline_spans(quote.trim())));
            index += 1;
            continue;
        }

        if let Some(item) = parse_list_item(line) {
            blocks.push(MarkdownBlock::ListItem(parse_inline_spans(item)));
            index += 1;
            continue;
        }

        blocks.push(MarkdownBlock::Paragraph(parse_inline_spans(line)));
        index += 1;
    }

    blocks
}

fn parse_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start();
    let level = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if level == 0 {
        return None;
    }
    let text = trimmed[level..].trim();
    if text.is_empty() {
        return None;
    }
    Some((level.min(6) as u8, text.to_string()))
}

fn parse_list_item(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("-"))
        .or_else(|| trimmed.strip_prefix("*"))
        .map(str::trim)
        .filter(|item| !item.is_empty())
}

fn parse_image(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("![")?;
    let (alt, rest) = rest.split_once("](")?;
    let url = rest.strip_suffix(')')?;
    if url.trim().is_empty() {
        return None;
    }
    Some((alt.to_string(), url.trim().to_string()))
}

fn parse_inline_spans(text: &str) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        let Some(marker) = next_marker(remaining) else {
            push_text(&mut spans, remaining);
            break;
        };
        if marker.start > 0 {
            push_text(&mut spans, &remaining[..marker.start]);
        }
        spans.push(marker.span);
        remaining = &remaining[marker.end..];
    }
    spans
}

struct Marker {
    start: usize,
    end: usize,
    span: InlineSpan,
}

fn next_marker(text: &str) -> Option<Marker> {
    [
        ("**", "**", InlineKind::Bold),
        ("__", "__", InlineKind::Underline),
        ("~~", "~~", InlineKind::Strike),
        ("`", "`", InlineKind::Code),
        ("*", "*", InlineKind::Italic),
        ("_", "_", InlineKind::Italic),
    ]
    .into_iter()
    .filter_map(|(open, close, kind)| marker_for(text, open, close, kind))
    .min_by_key(|marker| marker.start)
}

#[derive(Clone, Copy)]
enum InlineKind {
    Bold,
    Italic,
    Strike,
    Code,
    Underline,
}

fn marker_for(text: &str, open: &str, close: &str, kind: InlineKind) -> Option<Marker> {
    let start = text.find(open)?;
    let content_start = start + open.len();
    let relative_end = text[content_start..].find(close)?;
    let content_end = content_start + relative_end;
    if content_end == content_start {
        return None;
    }
    let content = text[content_start..content_end].to_string();
    let span = match kind {
        InlineKind::Bold => InlineSpan::Bold(content),
        InlineKind::Italic => InlineSpan::Italic(content),
        InlineKind::Strike => InlineSpan::Strike(content),
        InlineKind::Code => InlineSpan::Code(content),
        InlineKind::Underline => InlineSpan::Underline(content),
    };
    Some(Marker {
        start,
        end: content_end + close.len(),
        span,
    })
}

fn push_text(spans: &mut Vec<InlineSpan>, text: &str) {
    if !text.is_empty() {
        spans.push(InlineSpan::Text(text.to_string()));
    }
}

fn non_empty(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{InlineSpan, MarkdownBlock, MarkdownDocument};

    #[test]
    fn parses_markdown_blocks_used_by_local_t2i() {
        let document = MarkdownDocument::parse(
            "# Title\n> quote\n- item\n```rust\nlet x = 1;\n```\n![alt](https://example.test/a.png)",
        );

        assert!(matches!(
            document.blocks[0],
            MarkdownBlock::Heading { level: 1, .. }
        ));
        assert!(matches!(document.blocks[1], MarkdownBlock::Quote(_)));
        assert!(matches!(document.blocks[2], MarkdownBlock::ListItem(_)));
        assert!(matches!(
            document.blocks[3],
            MarkdownBlock::CodeBlock { .. }
        ));
        assert!(matches!(document.blocks[4], MarkdownBlock::Image { .. }));
    }

    #[test]
    fn parses_inline_style_spans_without_raster_coupling() {
        let document = MarkdownDocument::parse("hello **bold** `code` ~~gone~~");

        let MarkdownBlock::Paragraph(spans) = &document.blocks[0] else {
            panic!("paragraph expected");
        };
        assert!(spans.contains(&InlineSpan::Bold("bold".to_string())));
        assert!(spans.contains(&InlineSpan::Code("code".to_string())));
        assert!(spans.contains(&InlineSpan::Strike("gone".to_string())));
    }
}
