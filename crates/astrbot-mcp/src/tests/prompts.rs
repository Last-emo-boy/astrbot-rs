use crate::{
    McpContentBlock, McpJsonObject, McpMimeType, McpModelHint, McpSamplingInteractionState,
    McpSamplingMessage, McpSamplingPolicy, McpSamplingRequest, sampling,
};

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
        tools: Vec::new(),
        tool_choice: None,
    };

    let json = serde_json::to_value(request).expect("sampling should serialize");

    assert_eq!(json["systemPrompt"], "be concise");
    assert_eq!(json["maxTokens"], 128);
    assert_eq!(json["stopSequences"][0], "END");
    assert_eq!(json["messages"][0]["content"]["type"], "text");
}

#[test]
fn sampling_policy_requires_active_interaction_and_rejects_unsupported_paths() {
    let request = McpSamplingRequest::new(vec![McpSamplingMessage::user_text("summarize")]);
    let inactive = McpSamplingInteractionState {
        sampling_enabled: true,
        active_interaction: false,
        provider_available: true,
        unified_msg_origin: Some("umo-1".to_string()),
    };
    let error = McpSamplingPolicy::prepare_provider_request(&inactive, &request)
        .expect_err("inactive context should be rejected");
    assert!(error.to_string().contains("active AstrBot MCP interaction"));

    let mut include_context = request.clone();
    include_context.include_context = Some(sampling::McpIncludeContext::AllServers);
    let error = McpSamplingPolicy::prepare_provider_request(
        &McpSamplingInteractionState::active("umo-1"),
        &include_context,
    )
    .expect_err("include context should be rejected");
    assert!(error.to_string().contains("includeContext"));

    let mut with_tools = request.clone();
    with_tools.tools = vec![McpJsonObject::new().with("name", "search")];
    let error = McpSamplingPolicy::prepare_provider_request(
        &McpSamplingInteractionState::active("umo-1"),
        &with_tools,
    )
    .expect_err("tools should be rejected");
    assert!(error.to_string().contains("Tool-assisted sampling"));

    let image = McpSamplingRequest::new(vec![McpSamplingMessage {
        role: crate::McpSamplingRole::User,
        content: McpContentBlock::Image {
            data: "abc".to_string(),
            mime_type: McpMimeType::new("image/png").expect("mime"),
        },
    }]);
    let error = McpSamplingPolicy::prepare_provider_request(
        &McpSamplingInteractionState::active("umo-1"),
        &image,
    )
    .expect_err("image should be rejected");
    assert!(error.to_string().contains("Image sampling inputs"));

    let audio = McpSamplingRequest::new(vec![McpSamplingMessage {
        role: crate::McpSamplingRole::User,
        content: McpContentBlock::Audio {
            data: "abc".to_string(),
            mime_type: McpMimeType::new("audio/wav").expect("mime"),
        },
    }]);
    let error = McpSamplingPolicy::prepare_provider_request(
        &McpSamplingInteractionState::active("umo-1"),
        &audio,
    )
    .expect_err("audio should be rejected");
    assert!(error.to_string().contains("Audio sampling inputs"));
}

#[test]
fn sampling_policy_maps_text_messages_to_current_provider_request() {
    let mut request = McpSamplingRequest::new(vec![
        McpSamplingMessage::user_text("summarize"),
        McpSamplingMessage {
            role: crate::McpSamplingRole::Assistant,
            content: McpContentBlock::Text {
                text: "draft".to_string(),
            },
        },
    ]);
    request.system_prompt = Some("be concise".to_string());
    request.max_tokens = Some(64);
    request.temperature = Some(0.3);
    request.stop_sequences = vec!["END".to_string()];
    request.metadata = McpJsonObject::new().with("trace", "abc");

    let provider_request = McpSamplingPolicy::prepare_provider_request(
        &McpSamplingInteractionState::active("umo-1"),
        &request,
    )
    .expect("text sampling should map");

    assert_eq!(provider_request.unified_msg_origin, "umo-1");
    assert_eq!(provider_request.contexts[0].content, "summarize");
    assert_eq!(
        provider_request.contexts[1].role,
        crate::McpSamplingRole::Assistant
    );
    assert_eq!(provider_request.system_prompt, "be concise");
    assert_eq!(provider_request.stop_sequences, vec!["END".to_string()]);
}
