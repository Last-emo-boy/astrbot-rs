use std::sync::{Arc, Mutex};

use astrbot_core::{EventExecutor, MessageChain, MessageEvent, MessageSender, MessageSession};

use crate::{PipelineContext, PipelineControl, PipelineStageRegistry};

use super::support::{FailingInitStage, HandleRecordingStage, InitRecordingStage, NoopSink};

#[test]
fn scheduler_initializes_registered_stages_in_order() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let early_calls = calls.clone();
    let late_calls = calls.clone();
    let mut registry = PipelineStageRegistry::new();
    registry
        .register_stage("late", 20, move || InitRecordingStage {
            name: "late",
            calls: late_calls.clone(),
        })
        .expect("late registration should work");
    registry
        .register_stage("early", 10, move || InitRecordingStage {
            name: "early",
            calls: early_calls.clone(),
        })
        .expect("early registration should work");

    let scheduler = registry.build_scheduler(PipelineContext::new());
    scheduler.initialize().expect("scheduler should initialize");

    assert_eq!(
        *calls.lock().expect("init calls should lock"),
        vec!["early", "late"]
    );
}

#[test]
fn scheduler_returns_initialize_errors() {
    let mut registry = PipelineStageRegistry::new();
    registry
        .register_stage("failing", 10, || FailingInitStage)
        .expect("failing stage registration should work");

    let scheduler = registry.build_scheduler(PipelineContext::new());

    assert!(scheduler.initialize().is_err());
}

#[tokio::test]
async fn scheduler_stop_control_skips_later_stages() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let first_calls = calls.clone();
    let second_calls = calls.clone();
    let mut registry = PipelineStageRegistry::new();
    registry
        .register_stage("first", 10, move || HandleRecordingStage {
            name: "first",
            calls: first_calls.clone(),
            control: PipelineControl::Stop,
        })
        .expect("first registration should work");
    registry
        .register_stage("second", 20, move || HandleRecordingStage {
            name: "second",
            calls: second_calls.clone(),
            control: PipelineControl::Continue,
        })
        .expect("second registration should work");
    let scheduler = registry.build_scheduler(PipelineContext::new());
    let event = MessageEvent::new(
        "event-1",
        "test",
        "Test",
        MessageSession::new("test", "conversation-1"),
        MessageSender::new("user-1", None),
        MessageChain::plain("hello"),
        Arc::new(NoopSink),
    );

    scheduler
        .execute(event)
        .await
        .expect("scheduler should execute");

    assert_eq!(
        *calls.lock().expect("handle calls should lock"),
        vec!["first"]
    );
}
