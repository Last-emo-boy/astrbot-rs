use std::time::Duration;

use astrbot_core::{ProviderContentPart, ProviderContextMessage};
use astrbot_tool::ToolCallReferencePayload;

use crate::{
    AgentFeedbackEvent, AgentMessage, AgentMessageRole, AgentReferenceDecorator,
    AgentResponseEvent, AgentResponseEventKind, AgentResponseStats, AgentRunContext,
    AgentTokenUsage, AgentToolCall, InMemoryToolImageCache, ToolImageCachePort,
    ToolImageCacheRequest,
};

use super::support::event;

#[test]
fn agent_message_wraps_context_parts_and_tool_calls() {
    let message = AgentMessage::assistant_tool_call(
        AgentToolCall::new("call-1", "search").with_arguments("{\"q\":\"rust\"}"),
    );

    assert_eq!(message.role, AgentMessageRole::Assistant);
    assert!(message.parts.is_empty());
    assert!(message.is_valid());
    assert_eq!(message.tool_calls[0].name, "search");

    let provider_message: ProviderContextMessage =
        AgentMessage::tool("call-1", "result text").into();
    assert_eq!(provider_message.role, "tool");
    assert_eq!(
        provider_message.parts,
        vec![ProviderContentPart::text("result text")]
    );
}

#[test]
fn run_context_keeps_event_identity_messages_and_timeout_policy() {
    let mut context = AgentRunContext::new("state", &event("hello"))
        .with_tool_call_timeout(Duration::from_secs(0));
    context.push_message(AgentMessage::user("hello"));

    assert_eq!(context.context(), &"state");
    assert_eq!(context.event_id(), "event-1");
    assert_eq!(context.session_id(), "conversation-1");
    assert_eq!(context.tool_call_timeout(), Duration::from_secs(1));
    assert_eq!(context.messages().len(), 1);
}

#[test]
fn response_event_maps_feedback_and_stats_without_provider_dependency() {
    let event = AgentResponseEvent::from(AgentFeedbackEvent::streaming_delta("partial"));
    assert_eq!(event.kind, AgentResponseEventKind::Delta);
    assert_eq!(event.chain.plain_text(), "partial");

    let stats = AgentResponseStats::default()
        .with_token_usage(AgentTokenUsage::new(3, 4))
        .with_duration(Duration::from_millis(10))
        .with_time_to_first_token(Duration::from_millis(2));
    let event = AgentResponseEvent::stats(stats);

    assert_eq!(event.kind, AgentResponseEventKind::Stats);
    let stats = event.stats.expect("stats event should include stats");
    assert_eq!(stats.token_usage.expect("usage").total_tokens, 7);
    assert_eq!(stats.duration_ms, 10);
    assert_eq!(stats.time_to_first_token_ms, 2);
}

#[test]
fn reference_decorator_attaches_tool_refs_to_response_event_metadata() {
    let event = AgentResponseEvent::final_chain("Use <ref>abcd.1</ref>.");
    let tool_result = serde_json::json!({
        "results": [
            {
                "title": "AstrBot",
                "url": "https://astrbot.app",
                "snippet": "AstrBot docs",
                "index": "abcd.1"
            }
        ]
    })
    .to_string();

    let decorated = AgentReferenceDecorator::default().decorate_event(
        event,
        &[ToolCallReferencePayload::new(
            "web_search_bocha",
            tool_result,
        )],
    );

    let refs = decorated
        .references
        .expect("referenced event should carry refs");
    assert_eq!(refs.tool_refs.used.len(), 1);
    assert_eq!(refs.tool_refs.used[0].title.as_deref(), Some("AstrBot"));
}

#[tokio::test]
async fn in_memory_tool_image_cache_saves_reads_and_cleans_expired_images() {
    let cache = InMemoryToolImageCache::new(Duration::from_secs(60));
    let cached = cache
        .save_image(
            ToolImageCacheRequest::new("aW1hZ2U=", "call 1", "draw")
                .with_index(2)
                .with_mime_type("image/jpeg"),
        )
        .await
        .expect("image should cache");

    assert_eq!(cached.tool_call_id, "call 1");
    assert!(cached.uri.ends_with(".jpg"));
    assert_eq!(cache.len(), 1);

    let data = cache
        .get_image(&cached.uri, "")
        .await
        .expect("cache read should succeed")
        .expect("image should exist");
    assert_eq!(data.base64_data, "aW1hZ2U=");
    assert_eq!(data.mime_type, "image/jpeg");

    let expired_cache = InMemoryToolImageCache::new(Duration::from_secs(1));
    let expired = expired_cache
        .save_image(ToolImageCacheRequest::new("old", "call-2", "draw"))
        .await
        .expect("image should cache");
    tokio::time::sleep(Duration::from_millis(1100)).await;

    assert_eq!(
        expired_cache
            .get_image(&expired.uri, "image/png")
            .await
            .expect("cache read should succeed"),
        None
    );
    assert_eq!(
        expired_cache
            .cleanup_expired()
            .await
            .expect("cleanup should succeed"),
        1
    );
}
