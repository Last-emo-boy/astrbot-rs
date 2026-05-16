use std::time::Duration;

use astrbot_core::MessageChain;

use crate::{
    ActiveEventInterruption, ActiveEventRegistry, SessionLockManager, SessionWaitDecision,
    SessionWaiter, SessionWaitingEvent,
};

#[test]
fn session_waiter_records_history_and_finishes_without_platforms() {
    let mut waiter = SessionWaiter::new();

    assert_eq!(
        waiter.register("webchat:direct:user-1", Duration::from_secs(30), true),
        SessionWaitDecision::Waiting
    );
    assert_eq!(
        waiter.trigger(SessionWaitingEvent::new(
            "webchat:direct:user-1",
            MessageChain::plain("next"),
        )),
        SessionWaitDecision::Triggered
    );

    let history = waiter
        .history("webchat:direct:user-1")
        .expect("history should be available");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].plain_text(), "next");

    let registration = waiter
        .finish("webchat:direct:user-1")
        .expect("wait should finish");
    assert_eq!(registration.history().len(), 1);
    assert!(!waiter.has_wait("webchat:direct:user-1"));
}

#[test]
fn session_waiter_keep_zero_timeout_removes_wait() {
    let mut waiter = SessionWaiter::new();
    waiter.register("session", Duration::from_secs(30), false);

    assert_eq!(
        waiter.keep("session", Duration::ZERO, true),
        SessionWaitDecision::TimedOut
    );
    assert_eq!(
        waiter.trigger(SessionWaitingEvent::new(
            "session",
            MessageChain::plain("late")
        )),
        SessionWaitDecision::Missing
    );
}

#[tokio::test]
async fn session_lock_manager_serializes_per_session_work() {
    let locks = SessionLockManager::new();
    let first = locks.acquire("session-1").await;
    let second_locks = locks.clone();
    let blocked = tokio::spawn(async move {
        let guard = second_locks.acquire("session-1").await;
        guard.release().await;
        "acquired"
    });

    tokio::task::yield_now().await;
    assert!(!blocked.is_finished());
    assert_eq!(locks.tracked_session_count().await, 1);

    first.release().await;
    assert_eq!(blocked.await.expect("task should complete"), "acquired");
    assert_eq!(locks.tracked_session_count().await, 0);
}

#[test]
fn active_event_registry_tracks_stop_and_agent_stop_separately() {
    let mut registry = ActiveEventRegistry::new();
    registry.register("event-1", "session");
    registry.register("event-2", "session");
    registry.register("event-3", "other");

    assert_eq!(
        registry.interrupt_session(
            "session",
            ActiveEventInterruption::RequestAgentStop,
            Some("event-1"),
        ),
        1
    );
    assert!(
        registry
            .record("event-2")
            .expect("event should exist")
            .agent_stop_requested
    );
    assert!(
        !registry
            .record("event-1")
            .expect("event should exist")
            .agent_stop_requested
    );

    assert_eq!(
        registry.interrupt_session("session", ActiveEventInterruption::StopEvent, None),
        2
    );
    assert!(
        registry
            .record("event-1")
            .expect("event should exist")
            .stop_event_requested
    );
    assert_eq!(registry.session_event_count("session"), 2);
    registry.unregister("event-1");
    assert_eq!(registry.session_event_count("session"), 1);
    assert_eq!(registry.session_event_count("other"), 1);
}
