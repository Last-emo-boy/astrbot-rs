use std::net::TcpListener;
use std::sync::Arc;

use astrbot_metrics::InMemoryMetricSink;
use astrbot_observability::{
    ComponentKind, ComponentStatus, InMemoryLogBuffer, InMemoryStatusCollector, StatusEventSink,
};
use tokio::net::TcpStream;
use tokio::time::{Duration, sleep};

use crate::defaults::{DEFAULT_MOCK_PROVIDER_ID, DEFAULT_MOCK_RESPONSE};
use crate::{
    AstrbotRuntime, RuntimeChatProviderConfig, RuntimeConfig, RuntimePlatformConfig,
    RuntimeStatePolicyConfig,
};

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
async fn runtime_handle_records_platform_message_metrics() {
    let metrics = Arc::new(InMemoryMetricSink::new());
    let logs = Arc::new(InMemoryLogBuffer::new(8));
    let runtime = AstrbotRuntime::initialize(RuntimeConfig::default())
        .expect("runtime should initialize")
        .with_metric_sink(metrics.clone())
        .with_log_buffer(logs.clone());
    let handle = runtime.start();

    handle
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");

    let sent = wait_for_sent_messages(&handle, 1).await;
    assert_eq!(sent.len(), 1);

    let events = metrics.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].platform_id.as_deref(), Some("mock"));
    assert_eq!(events[0].platform_type.as_deref(), Some("mock"));
    assert_eq!(events[0].count, 1);
    let snapshot = logs.snapshot(None, 8).await;
    assert_eq!(snapshot.entries.len(), 1);
    assert!(snapshot.entries[0].message.contains("user-1: hello"));

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
    let metrics = Arc::new(InMemoryMetricSink::new());
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
        .with_metric_sink(metrics.clone())
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
    assert_eq!(metrics.events().len(), 1);

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

#[tokio::test]
async fn runtime_restart_releases_real_adapter_port_and_preserves_provider_preference() {
    let port = unused_local_port();
    let config = RuntimeConfig {
        default_chat_provider_id: "default-provider".to_string(),
        chat_providers: vec![
            RuntimeChatProviderConfig::mock("default-provider", "default-response"),
            RuntimeChatProviderConfig::mock("preferred-provider", "preferred-response"),
        ],
        platforms: vec![
            RuntimePlatformConfig::onebot("onebot")
                .with_option_string("ws_reverse_host", "127.0.0.1")
                .with_option_u16("ws_reverse_port", port),
        ],
        ..RuntimeConfig::default()
    };
    let handle = AstrbotRuntime::initialize(config.clone())
        .expect("runtime should initialize")
        .start();
    wait_for_tcp_port(port).await;
    handle
        .provider_preference()
        .set_preferred_chat_provider("conversation-1", "preferred-provider")
        .await
        .expect("provider preference should be stored");

    let restarted = handle
        .restart(config)
        .await
        .expect("runtime should restart with the same adapter port");
    wait_for_tcp_port(port).await;

    let preferences = restarted
        .provider_preference()
        .snapshot()
        .await
        .expect("provider preferences should be readable");
    assert_eq!(
        preferences.get("conversation-1").map(String::as_str),
        Some("preferred-provider")
    );

    restarted
        .stop()
        .await
        .expect("restarted runtime should stop");
}

fn unused_local_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("ephemeral port should bind");
    let port = listener
        .local_addr()
        .expect("ephemeral listener should have local addr")
        .port();
    drop(listener);
    port
}

async fn wait_for_tcp_port(port: u16) {
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("port {port} did not start accepting connections");
}
