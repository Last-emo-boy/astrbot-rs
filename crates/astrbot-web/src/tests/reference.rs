use astrbot_tool::{ToolReferenceItem, ToolReferenceSet};

use crate::WebChatReferenceResponse;

#[test]
fn webchat_reference_response_serializes_tool_refs_without_extraction_logic() {
    let refs = ToolReferenceSet::new(vec![
        ToolReferenceItem::new("abcd.1")
            .with_url("https://astrbot.app")
            .with_title("AstrBot")
            .with_snippet("AstrBot docs")
            .with_favicon("https://astrbot.app/favicon.ico"),
    ]);

    let response = WebChatReferenceResponse::from(refs);
    let payload = serde_json::to_value(&response).expect("refs should serialize");

    assert_eq!(payload["used"][0]["index"], "abcd.1");
    assert_eq!(payload["used"][0]["url"], "https://astrbot.app");
    assert_eq!(
        payload["used"][0]["favicon"],
        "https://astrbot.app/favicon.ico"
    );
}
