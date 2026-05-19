use std::sync::Arc;
use std::time::Duration;

use astrbot_core::{MessageChain, MessageComponent, MessageEvent};
use tokio::time;

use crate::{EmptyMentionPolicy, SessionWaiterRegistry};

use super::event;

#[tokio::test]
async fn session_waiter_receives_next_event_for_same_session() {
    let registry = Arc::new(SessionWaiterRegistry::new());
    let waiting = registry.clone();
    let waiter = tokio::spawn(async move {
        waiting
            .wait_for_next("user", Duration::from_secs(1))
            .await
            .expect("waiter should complete")
    });

    wait_until_registered(&registry).await;

    let incoming = event("follow up");
    assert!(registry.trigger(&incoming).await);

    let delivered = waiter
        .await
        .expect("waiter task should join")
        .expect("event should be delivered");
    assert_eq!(delivered.message.plain_text(), "follow up");
    assert_eq!(registry.waiter_count().await, 0);
}

#[tokio::test]
async fn session_waiter_times_out_and_cleans_registration() {
    let registry = SessionWaiterRegistry::new();

    let delivered = registry
        .wait_for_next("console", Duration::from_millis(1))
        .await
        .expect("timeout is not an error");

    assert!(delivered.is_none());
    assert_eq!(registry.waiter_count().await, 0);
}

#[test]
fn empty_self_mention_waits_for_followup() {
    let policy = EmptyMentionPolicy::new();
    let event = event_with_message(MessageChain::new(vec![MessageComponent::mention("bot")]))
        .with_self_id("bot");

    assert!(policy.should_wait_for_followup(&event));
}

#[test]
fn wake_prefix_only_waits_for_followup() {
    let policy = EmptyMentionPolicy::new().with_wake_prefix("/");

    assert!(policy.should_wait_for_followup(&event("/")));
    assert!(!policy.should_wait_for_followup(&event("/help")));
}

#[test]
fn redispatch_followup_mentions_self_and_marks_wake() {
    let policy = EmptyMentionPolicy::new();
    let event = event("hello").with_self_id("bot");

    let redispatched = policy.redispatch_followup(&event);

    assert!(redispatched.message.mentions_user("bot"));
    assert!(redispatched.is_at_or_wake_command());
    assert_eq!(redispatched.message.plain_text(), "hello");
}

fn event_with_message(message: MessageChain) -> MessageEvent {
    let mut event = event("");
    event.message = message;
    event
}

async fn wait_until_registered(registry: &SessionWaiterRegistry) {
    let deadline = time::Instant::now() + Duration::from_secs(1);
    while registry.waiter_count().await == 0 {
        assert!(
            time::Instant::now() < deadline,
            "waiter should register before timeout"
        );
        time::sleep(Duration::from_millis(1)).await;
    }
}
