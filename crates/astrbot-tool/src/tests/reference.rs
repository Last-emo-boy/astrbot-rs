use serde_json::json;

use crate::{ToolCallReferencePayload, ToolReferenceExtractor};

#[test]
fn tool_reference_extractor_matches_only_used_web_search_refs() {
    let result = json!({
        "results": [
            {
                "title": "Rust",
                "url": "https://www.rust-lang.org",
                "snippet": "Rust language",
                "index": "abcd.1"
            },
            {
                "title": "Ignored",
                "url": "https://example.test/ignored",
                "snippet": "Unused",
                "index": "abcd.2"
            }
        ]
    })
    .to_string();
    let extractor = ToolReferenceExtractor::default().with_favicon(
        "https://www.rust-lang.org",
        "https://www.rust-lang.org/favicon.ico",
    );

    let refs = extractor.extract_from_tool_calls(
        "See <ref>abcd.1</ref>, repeat <ref>abcd.1</ref>, missing <ref>none</ref>.",
        &[ToolCallReferencePayload::new("web_search_tavily", result)],
    );

    assert_eq!(refs.used.len(), 1);
    assert_eq!(refs.used[0].index, "abcd.1");
    assert_eq!(
        refs.used[0].url.as_deref(),
        Some("https://www.rust-lang.org")
    );
    assert_eq!(
        refs.used[0].favicon.as_deref(),
        Some("https://www.rust-lang.org/favicon.ico")
    );
}

#[test]
fn tool_reference_extractor_ignores_unsupported_tools_and_invalid_json() {
    let extractor = ToolReferenceExtractor::default();

    let refs = extractor.extract_from_tool_calls(
        "See <ref>abcd.1</ref>.",
        &[
            ToolCallReferencePayload::new("web_search", "{\"results\": []}"),
            ToolCallReferencePayload::new("web_search_bocha", "{not-json"),
        ],
    );

    assert!(refs.is_empty());
}

#[test]
fn tool_reference_extractor_merges_tavily_and_bocha_json_refs() {
    let tavily = json!({
        "results": [
            {
                "title": "Tavily",
                "url": "https://tavily.example",
                "snippet": "Tavily snippet",
                "index": "tvly.1"
            }
        ]
    })
    .to_string();
    let bocha = json!({
        "results": [
            {
                "title": "Bocha",
                "url": "https://bocha.example",
                "snippet": "Bocha snippet",
                "index": "boch.1"
            }
        ]
    })
    .to_string();

    let refs = ToolReferenceExtractor::default().extract_from_tool_calls(
        "Use <ref>boch.1</ref> and <ref>tvly.1</ref>.",
        &[
            ToolCallReferencePayload::new("web_search_tavily", tavily),
            ToolCallReferencePayload::new("web_search_bocha", bocha),
        ],
    );

    assert_eq!(refs.used.len(), 2);
    assert_eq!(refs.used[0].index, "boch.1");
    assert_eq!(refs.used[0].title.as_deref(), Some("Bocha"));
    assert_eq!(refs.used[1].index, "tvly.1");
    assert_eq!(refs.used[1].title.as_deref(), Some("Tavily"));
}
