use crate::{McpContentBlock, McpReadResourceResult, McpToolResultStatus, McpUri};

#[test]
fn resources_shape_read_result_as_tool_text() {
    let uri = McpUri::new("file:///tmp/readme.txt").expect("uri");
    let result = McpReadResourceResult::text(uri.clone(), "hello");

    let shaped = result.into_tool_result("docs", &uri);

    assert_eq!(shaped.status, McpToolResultStatus::Completed);
    assert!(!shaped.is_error);
    assert_eq!(
        shaped.content,
        vec![McpContentBlock::Text {
            text: "MCP text resource from server 'docs':\nURI: file:///tmp/readme.txt\n\nhello"
                .to_string()
        }]
    );
}
