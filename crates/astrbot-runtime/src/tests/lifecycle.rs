use std::sync::Arc;

use astrbot_observability::{
    ComponentKind, ComponentStatus, InMemoryStatusCollector, StatusEventSink,
};

use crate::defaults::{DEFAULT_MOCK_PROVIDER_ID, DEFAULT_MOCK_RESPONSE};
use crate::{AstrbotRuntime, RuntimeChatProviderConfig, RuntimeConfig, RuntimeStatePolicyConfig};

use super::wait_for_sent_messages;

#[tokio::test]
async fn runtime_handle_processes_events_and_stops() {
    let runtime =
        AstrbotRuntime::initialize(RuntimeConfig::default()).expect("runtime should initialize");
    let handle = runtime.start();

    handle
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");

    let sent = wait_for_sent_messages(&handle, 1).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), DEFAULT_MOCK_RESPONSE);
    assert_eq!(handle.platform_manager().platform_count(), 1);

    handle.stop().await.expect("runtime should stop");
}

#[tokio::test]
async fn runtime_handle_emits_status_events_without_changing_stop_behavior() {
    let sink = Arc::new(InMemoryStatusCollector::new());
    let runtime = AstrbotRuntime::initialize(RuntimeConfig::default())
        .expect("runtime should initialize")
        .with_status_sink(sink.clone() as Arc<dyn StatusEventSink>);
    let handle = runtime.start();

    handle
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    let sent = wait_for_sent_messages(&handle, 1).await;
    assert_eq!(sent.len(), 1);

    handle.stop().await.expect("runtime should stop");

    let events = sink.events();
    assert!(events.iter().any(|event| {
        event.component == ComponentKind::Runtime && event.status == ComponentStatus::Starting
    }));
    assert!(events.iter().any(|event| {
        event.component == ComponentKind::Runtime && event.status == ComponentStatus::Stopped
    }));
    assert!(events.iter().any(|event| {
        event.component == ComponentKind::Task && event.status == ComponentStatus::Stopped
    }));
    assert!(events.iter().any(|event| {
        event.component == ComponentKind::Provider && event.status == ComponentStatus::Stopped
    }));
    assert!(events.iter().any(|event| {
        event.component == ComponentKind::Platform && event.status == ComponentStatus::Stopped
    }));
}

#[tokio::test]
async fn runtime_handle_can_restart_with_new_config() {
    let runtime =
        AstrbotRuntime::initialize(RuntimeConfig::default()).expect("runtime should initialize");
    let handle = runtime.start();

    handle
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    let sent = wait_for_sent_messages(&handle, 1).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), DEFAULT_MOCK_RESPONSE);

    let restarted = handle
        .restart(
            RuntimeConfig::new(vec![RuntimeChatProviderConfig::mock(
                DEFAULT_MOCK_PROVIDER_ID,
                "after-restart",
            )])
            .with_default_chat_provider_id(DEFAULT_MOCK_PROVIDER_ID),
        )
        .await
        .expect("runtime should restart");

    restarted
        .emit_mock_text("event-2", "conversation-1", "user-1", "hello again")
        .await
        .expect("event should enter restarted runtime");

    let sent = wait_for_sent_messages(&restarted, 1).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "after-restart");

    restarted
        .stop()
        .await
        .expect("restarted runtime should stop");
}

#[tokio::test]
async fn runtime_restart_preserves_provider_preference_by_default() {
    let config = RuntimeConfig {
        default_chat_provider_id: "default-provider".to_string(),
        chat_providers: vec![
            RuntimeChatProviderConfig::mock("default-provider", "default-response"),
            RuntimeChatProviderConfig::mock("preferred-provider", "preferred-response"),
        ],
        ..RuntimeConfig::default()
    };
    let handle = AstrbotRuntime::initialize(config.clone())
        .expect("runtime should initialize")
        .start();
    handle
        .provider_preference()
        .set_preferred_chat_provider("conversation-1", "preferred-provider")
        .await
        .expect("provider preference should be stored");

    let restarted = handle
        .restart(config)
        .await
        .expect("runtime should restart");
    restarted
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter restarted runtime");

    let sent = wait_for_sent_messages(&restarted, 1).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "preferred-response");

    restarted
        .stop()
        .await
        .expect("restarted runtime should stop");
}

#[tokio::test]
async fn runtime_restart_can_discard_provider_preference() {
    let config = RuntimeConfig {
        default_chat_provider_id: "default-provider".to_string(),
        chat_providers: vec![
            RuntimeChatProviderConfig::mock("default-provider", "default-response"),
            RuntimeChatProviderConfig::mock("preferred-provider", "preferred-response"),
        ],
        state_policy: RuntimeStatePolicyConfig {
            preserve_provider_preference_on_restart: false,
        },
        ..RuntimeConfig::default()
    };
    let handle = AstrbotRuntime::initialize(config.clone())
        .expect("runtime should initialize")
        .start();
    handle
        .provider_preference()
        .set_preferred_chat_provider("conversation-1", "preferred-provider")
        .await
        .expect("provider preference should be stored");

    let restarted = handle
        .restart(config)
        .await
        .expect("runtime should restart");
    restarted
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter restarted runtime");

    let sent = wait_for_sent_messages(&restarted, 1).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "default-response");

    restarted
        .stop()
        .await
        .expect("restarted runtime should stop");
}
