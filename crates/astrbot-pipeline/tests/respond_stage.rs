use std::sync::Arc;

use astrbot_core::{
    EventExecutor, MessageChain, MessageComponent, MessageEvent, MessageEventResult, MessageSender,
    MessageSession, MessageStream,
};
use astrbot_pipeline::{
    PipelineContext, PipelineControl, PipelineScheduler, PipelineStage, stages::RespondStage,
};
use astrbot_platform::RecordingSink;

#[tokio::test]
async fn respond_stage_skips_empty_plain_result() {
    let sink = Arc::new(RecordingSink::default());
    let scheduler = respond_scheduler();

    let mut event = direct_event("input", sink.clone());
    event.set_result(MessageEventResult::general(MessageChain::plain("   ")));

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn respond_stage_removes_empty_plain_before_send() {
    let sink = Arc::new(RecordingSink::default());
    let scheduler = respond_scheduler();

    let mut event = direct_event("input", sink.clone());
    event.set_result(MessageEventResult::general(MessageChain::new(vec![
        MessageComponent::plain(" "),
        MessageComponent::image("https://example.test/image.png"),
        MessageComponent::plain("hello"),
    ])));

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    let sent = sink.messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].chain.components(),
        &[
            MessageComponent::image("https://example.test/image.png"),
            MessageComponent::plain("hello"),
        ]
    );
}

#[tokio::test]
async fn respond_stage_skips_reply_and_mention_only_result() {
    let sink = Arc::new(RecordingSink::default());
    let scheduler = respond_scheduler();

    let mut event = direct_event("input", sink.clone());
    event.set_result(MessageEventResult::general(MessageChain::new(vec![
        MessageComponent::reply("message-1", "quoted text"),
        MessageComponent::mention("user-1"),
        MessageComponent::mention_all(),
    ])));

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn respond_stage_keeps_non_empty_media_components() {
    let sink = Arc::new(RecordingSink::default());
    let scheduler = respond_scheduler();

    let mut event = direct_event("input", sink.clone());
    event.set_result(MessageEventResult::general(MessageChain::new(vec![
        MessageComponent::image("https://example.test/image.png"),
        MessageComponent::record("https://example.test/audio.ogg"),
        MessageComponent::video("https://example.test/video.mp4"),
        MessageComponent::file("report.pdf", "https://example.test/report.pdf"),
    ])));

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    let sent = sink.messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.components().len(), 4);
}

#[tokio::test]
async fn respond_stage_removes_empty_media_but_sends_remaining_content() {
    let sink = Arc::new(RecordingSink::default());
    let scheduler = respond_scheduler();

    let mut event = direct_event("input", sink.clone());
    event.set_result(MessageEventResult::general(MessageChain::new(vec![
        MessageComponent::image(" "),
        MessageComponent::record(""),
        MessageComponent::video("https://example.test/video.mp4"),
        MessageComponent::file("empty.pdf", " "),
    ])));

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    let sent = sink.messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].chain.components(),
        &[MessageComponent::video("https://example.test/video.mp4")]
    );
}

#[tokio::test]
async fn respond_stage_stopped_empty_result_still_stops_pipeline() {
    let sink = Arc::new(RecordingSink::default());
    let mut event = direct_event("input", sink.clone());
    event.set_result(MessageEventResult::general(MessageChain::plain(" ")).stop());

    let control = RespondStage
        .handle(&mut event, &PipelineContext::new())
        .await
        .expect("respond should handle stopped empty result");

    assert_eq!(control, PipelineControl::Stop);
    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn respond_stage_streaming_result_uses_streaming_sink() {
    let sink = Arc::new(RecordingSink::default());
    let scheduler = respond_scheduler();

    let mut event = direct_event("input", sink.clone());
    event.set_result(MessageEventResult::streaming(MessageStream::new(vec![
        MessageChain::plain("first"),
        MessageChain::plain("second"),
    ])));

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    assert!(sink.messages().await.is_empty());
    let streamed = sink.streaming_messages().await;
    assert_eq!(streamed.len(), 1);
    assert_eq!(streamed[0].stream.chunks()[0].plain_text(), "first");
    assert_eq!(streamed[0].stream.chunks()[1].plain_text(), "second");
}

#[tokio::test]
async fn respond_stage_stopped_streaming_result_still_stops_pipeline() {
    let sink = Arc::new(RecordingSink::default());
    let mut event = direct_event("input", sink.clone());
    event.set_result(MessageEventResult::streaming(MessageStream::from_chunk("first")).stop());

    let control = RespondStage
        .handle(&mut event, &PipelineContext::new())
        .await
        .expect("respond should handle stopped streaming result");

    assert_eq!(control, PipelineControl::Stop);
    assert!(sink.messages().await.is_empty());
    assert_eq!(sink.streaming_messages().await.len(), 1);
}

#[tokio::test]
async fn respond_stage_streaming_finish_marks_event_and_skips_duplicates() {
    let sink = Arc::new(RecordingSink::default());
    let mut event = direct_event("input", sink.clone());
    event.set_result(MessageEventResult::streaming_finish(MessageChain::plain(
        "final",
    )));

    let control = RespondStage
        .handle(&mut event, &PipelineContext::new())
        .await
        .expect("respond should handle streaming finish");

    assert_eq!(control, PipelineControl::Continue);
    assert!(event.is_streaming_finished());
    assert!(sink.messages().await.is_empty());
    assert!(sink.streaming_messages().await.is_empty());

    event.set_result(MessageEventResult::llm("duplicate"));
    RespondStage
        .handle(&mut event, &PipelineContext::new())
        .await
        .expect("streaming-finished event should skip duplicate result");

    assert!(sink.messages().await.is_empty());
    assert!(sink.streaming_messages().await.is_empty());
}

fn respond_scheduler() -> PipelineScheduler {
    PipelineScheduler::new(PipelineContext::new()).with_stage(RespondStage)
}

fn direct_event(text: impl Into<String>, sink: Arc<RecordingSink>) -> MessageEvent {
    MessageEvent::new(
        "event-1",
        "mock",
        "Mock Platform",
        MessageSession::new("mock", "conversation-1"),
        MessageSender::new("user-1", None),
        MessageChain::plain(text),
        sink,
    )
}
