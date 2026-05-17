use crate::{McpJsonObject, McpModelHint, McpSamplingMessage, McpSamplingRequest, sampling};

#[test]
fn prompts_and_sampling_use_mcp_json_field_names() {
    let request = McpSamplingRequest {
        messages: vec![McpSamplingMessage::user_text("summarize")],
        system_prompt: Some("be concise".to_string()),
        max_tokens: Some(128),
        temperature: Some(0.2),
        stop_sequences: vec!["END".to_string()],
        include_context: Some(sampling::McpIncludeContext::None),
        metadata: McpJsonObject::new().with("trace", "abc"),
        model_preferences: vec![McpModelHint::new("gpt")],
    };

    let json = serde_json::to_value(request).expect("sampling should serialize");

    assert_eq!(json["systemPrompt"], "be concise");
    assert_eq!(json["maxTokens"], 128);
    assert_eq!(json["stopSequences"][0], "END");
    assert_eq!(json["messages"][0]["content"]["type"], "text");
}
