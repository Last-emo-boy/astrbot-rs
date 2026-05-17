use std::sync::Arc;

use async_trait::async_trait;

use crate::Result;

use super::{
    ForwardMessageReference, MessageChain, MessageComponent, MessageEvent, MessageEventResult,
    MessageSender, MessageSession, MessageSink, MessageStream, ProviderContentPart,
    ProviderContextMessage, ProviderRequest, ProviderToolPlaceholder, QuotedImageReference,
    QuotedImageReferenceKind, QuotedMessage, ResultContentType,
};

struct NoopSink;

#[async_trait]
impl MessageSink for NoopSink {
    async fn send(&self, _session: &MessageSession, _chain: MessageChain) -> Result<()> {
        Ok(())
    }
}

#[test]
fn message_chain_extracts_text_and_image_urls() {
    let mut chain = MessageChain::new(vec![
        MessageComponent::reply("message-1", "quoted text"),
        MessageComponent::plain("describe"),
        MessageComponent::mention("bot-1"),
        MessageComponent::mention_all(),
        MessageComponent::image(" https://example.test/image.png "),
        MessageComponent::image(" "),
        MessageComponent::record("https://example.test/audio.ogg"),
        MessageComponent::video("https://example.test/video.mp4"),
        MessageComponent::file("report.pdf", "https://example.test/report.pdf"),
        MessageComponent::reply_to_sender("message-2", "bot reply", "bot-1"),
    ]);

    assert_eq!(chain.plain_text(), "describe");
    assert!(chain.prefix_first_plain("[bot] "));
    assert_eq!(chain.plain_text(), "[bot] describe");
    assert_eq!(
        chain.image_urls(),
        vec!["https://example.test/image.png".to_string()]
    );
    assert_eq!(
        chain.components()[6],
        MessageComponent::record("https://example.test/audio.ogg")
    );
    if let MessageComponent::Record { url } = &mut chain.components_mut()[6] {
        *url = "file:///tmp/audio.ogg".to_string();
    }
    assert_eq!(
        chain.components()[6],
        MessageComponent::record("file:///tmp/audio.ogg")
    );
    assert!(chain.mentions_user("bot-1"));
    assert!(chain.mentions_all());
    assert!(chain.replies_to_user("bot-1"));
    assert!(!MessageComponent::file("report.pdf", "https://example.test/report.pdf").is_empty());
    assert!(MessageComponent::video(" ").is_empty());
    assert!(
        MessageChain::new(vec![MessageComponent::reply("message-1", "quoted text")]).is_empty()
    );

    assert!(MessageComponent::image("https://example.test/image.png").has_sendable_content());
    assert!(!MessageComponent::mention("user-1").has_sendable_content());
    assert!(MessageComponent::mention("user-1").is_valid_send_component());
    assert!(!MessageComponent::mention(" ").is_valid_send_component());

    let mut outbound = MessageChain::new(vec![
        MessageComponent::reply("message-1", "quoted text"),
        MessageComponent::mention("user-1"),
        MessageComponent::plain(" "),
        MessageComponent::image("https://example.test/image.png"),
        MessageComponent::file("empty.pdf", " "),
    ]);
    assert!(outbound.has_sendable_content());
    outbound.retain_valid_send_components();
    assert_eq!(
        outbound.components(),
        &[
            MessageComponent::reply("message-1", "quoted text"),
            MessageComponent::mention("user-1"),
            MessageComponent::image("https://example.test/image.png"),
        ]
    );
    assert!(outbound.into_sendable().is_some());
    assert!(
        MessageChain::new(vec![MessageComponent::mention_all()])
            .into_sendable()
            .is_none()
    );
}

#[test]
fn provider_request_builds_from_event_and_accepts_placeholders() {
    let event = MessageEvent::new(
        "event-1",
        "mock",
        "Mock Platform",
        MessageSession::new("mock", "conversation-1"),
        MessageSender::new("user-1", None),
        MessageChain::new(vec![
            MessageComponent::plain("describe"),
            MessageComponent::image("https://example.test/image.png"),
        ]),
        Arc::new(NoopSink),
    );

    let request = ProviderRequest::from_event(&event)
        .with_provider_id("openai")
        .with_system_prompt("be concise")
        .with_model("vision")
        .with_wake_prefix("llm")
        .with_context(ProviderContextMessage::text("assistant", "previous"))
        .with_extra_user_content_part(ProviderContentPart::text("extra"))
        .with_tool_placeholder(ProviderToolPlaceholder::new("search"));

    assert_eq!(request.prompt.as_deref(), Some("describe"));
    assert_eq!(request.session_id.as_deref(), Some("conversation-1"));
    assert_eq!(
        request.image_urls,
        vec!["https://example.test/image.png".to_string()]
    );
    assert!(request.has_user_content());
    assert_eq!(request.provider_id.as_deref(), Some("openai"));
    assert_eq!(request.contexts.len(), 1);
    assert_eq!(request.extra_user_content_parts.len(), 1);
    assert_eq!(request.tool_placeholders.len(), 1);
}

#[test]
fn quoted_message_domain_models_text_images_and_forward_refs() {
    let quote = QuotedMessage::new()
        .with_message_id("msg-1")
        .with_sender_name("Alice")
        .with_text("quoted text")
        .with_image_ref(QuotedImageReference::url("https://example.test/a.png"))
        .with_image_ref(QuotedImageReference::url("https://example.test/a.png"))
        .with_forward_ref(ForwardMessageReference::new("forward-1").with_preview_text("nested"));

    assert_eq!(quote.message_id.as_deref(), Some("msg-1"));
    assert_eq!(quote.text.as_deref(), Some("quoted text"));
    assert_eq!(quote.image_refs().len(), 1);
    assert_eq!(quote.image_refs()[0].kind, QuotedImageReferenceKind::Url);
    assert_eq!(
        quote.image_ref_values(),
        vec!["https://example.test/a.png".to_string()]
    );
    assert_eq!(quote.forward_refs().len(), 1);
    assert!(quote.has_content());
}

#[test]
fn message_stream_builds_streaming_results() {
    let mut stream = MessageStream::from_chunk("first");
    stream.push(MessageChain::plain("second"));

    assert_eq!(stream.chunks().len(), 2);
    assert!(!stream.is_empty());
    assert_eq!(stream.clone().into_chunks()[0].plain_text(), "first");

    let result = MessageEventResult::streaming(stream.clone());
    assert!(result.is_streaming());
    assert_eq!(result.content_type, ResultContentType::Streaming);
    assert_eq!(result.stream.as_ref(), Some(&stream));

    let replaced = MessageEventResult::general("fallback").with_stream(stream.clone());
    assert_eq!(replaced.stream.as_ref(), Some(&stream));

    let finish = MessageEventResult::streaming_finish("final");
    assert!(finish.is_streaming_finish());
    assert_eq!(finish.chain.plain_text(), "final");

    assert!(MessageStream::new(vec![MessageChain::plain(" ")]).is_empty());
}
