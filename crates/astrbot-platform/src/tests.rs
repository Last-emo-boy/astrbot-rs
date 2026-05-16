use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use astrbot_core::{
    AstrbotError, MessageChain, MessageSession, MessageSink, MessageStream, Result,
};
use async_trait::async_trait;
use tokio::sync::mpsc;

use super::*;
#[test]
fn builtins_register_mock_platform_type() {
    let registry = PlatformRegistry::with_builtin_platforms();

    assert!(registry.has_platform(MOCK_PLATFORM_TYPE));
    assert!(registry.has_platform(CONSOLE_PLATFORM_TYPE));
    assert!(registry.has_platform(WEBCHAT_PLATFORM_TYPE));
    assert!(registry.has_platform(ONEBOT_PLATFORM_TYPE));
}

#[test]
fn duplicate_platform_type_is_rejected() {
    let mut registry = PlatformRegistry::new();
    registry
        .register_platform("mock", |_config, _ctx| {
            Err(AstrbotError::Platform("unused".to_string()))
        })
        .expect("first registration should work");

    let duplicate = registry.register_platform("mock", |_config, _ctx| {
        Err(AstrbotError::Platform("unused".to_string()))
    });

    assert!(duplicate.is_err());
}

#[tokio::test]
async fn manager_builds_enabled_mock_platforms() {
    let registry = PlatformRegistry::with_builtin_platforms();
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let manager = PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig::mock("test-mock").with_name("Test Mock")],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build");

    assert_eq!(manager.platform_count(), 1);
    let adapter = manager
        .adapter("test-mock")
        .expect("configured adapter should exist");
    assert_eq!(adapter.id(), "test-mock");
    assert_eq!(adapter.name(), "Test Mock");

    let platform = manager
        .mock_platform("test-mock")
        .expect("mock platform should be available");
    platform
        .emit_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("mock platform should send event");

    let event = event_rx.recv().await.expect("event should be queued");
    assert_eq!(event.platform_id, "test-mock");
    assert_eq!(event.platform_name, "Test Mock");
    assert_eq!(event.message.plain_text(), "hello");
}

#[test]
fn manager_builds_enabled_console_platforms() {
    let registry = PlatformRegistry::with_builtin_platforms();
    let (event_tx, _event_rx) = mpsc::channel(1);
    let manager = PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig::console("console").with_name("Console")],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build");

    assert_eq!(manager.platform_count(), 1);
    let adapter = manager
        .adapter("console")
        .expect("configured adapter should exist");
    assert_eq!(adapter.id(), "console");
    assert_eq!(adapter.name(), "Console");
    assert!(manager.recording_sink("console").is_some());
}

#[test]
fn manager_builds_enabled_webchat_platforms() {
    let registry = PlatformRegistry::with_builtin_platforms();
    let (event_tx, _event_rx) = mpsc::channel(1);
    let manager = PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig::webchat("webchat").with_name("WebChat")],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build");

    assert_eq!(manager.platform_count(), 1);
    let adapter = manager
        .adapter("webchat")
        .expect("configured adapter should exist");
    assert_eq!(adapter.id(), "webchat");
    assert_eq!(adapter.name(), "WebChat");
    assert!(manager.webchat_platform("webchat").is_some());
    assert!(manager.recording_sink("webchat").is_some());
}

#[test]
fn manager_builds_enabled_onebot_platforms() {
    let registry = PlatformRegistry::with_builtin_platforms();
    let (event_tx, _event_rx) = mpsc::channel(1);
    let manager = PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig::onebot("onebot").with_name("OneBot")],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build");

    assert_eq!(manager.platform_count(), 1);
    let adapter = manager
        .adapter("onebot")
        .expect("configured adapter should exist");
    assert_eq!(adapter.id(), "onebot");
    assert_eq!(adapter.name(), "OneBot");
    assert!(manager.onebot_platform("onebot").is_some());
    assert!(manager.recording_sink("onebot").is_some());
}

#[tokio::test]
async fn webchat_platform_submits_text_events() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = WebChatPlatform::with_identity("webchat", "WebChat", event_tx, sink);

    let event_id = platform
        .submit_text("conversation-1", "user-1", "hello webchat")
        .await
        .expect("webchat input should submit event");

    let event = event_rx.recv().await.expect("event should be queued");
    assert_eq!(event.id, event_id);
    assert_eq!(event.platform_id, "webchat");
    assert_eq!(event.sender.id, "user-1");
    assert_eq!(event.session.conversation_id, "conversation-1");
    assert_eq!(event.message.plain_text(), "hello webchat");
}

#[tokio::test]
async fn webchat_platform_submits_image_only_events() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = WebChatPlatform::with_identity("webchat", "WebChat", event_tx, sink);

    platform
        .submit_message(
            "conversation-1",
            "user-1",
            "",
            vec!["https://example.test/image.png".to_string()],
        )
        .await
        .expect("webchat image input should submit event");

    let event = event_rx.recv().await.expect("event should be queued");
    assert_eq!(event.platform_id, "webchat");
    assert_eq!(event.sender.id, "user-1");
    assert_eq!(event.session.conversation_id, "conversation-1");
    assert_eq!(event.message.plain_text(), "");
    assert_eq!(
        event.message.image_urls(),
        vec!["https://example.test/image.png".to_string()]
    );
}

#[tokio::test]
async fn webchat_platform_rejects_empty_message_chains() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = WebChatPlatform::with_identity("webchat", "WebChat", event_tx, sink);

    let result = platform
        .submit_chain("conversation-1", "user-1", MessageChain::default())
        .await;

    assert!(matches!(result, Err(AstrbotError::EmptyMessage)));
    assert!(event_rx.try_recv().is_err());
}

#[tokio::test]
async fn webchat_platform_filters_messages_by_conversation() {
    let (event_tx, _event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = WebChatPlatform::with_identity("webchat", "WebChat", event_tx, sink.clone());
    let session_a = MessageSession::new("webchat", "conversation-a");
    let session_b = MessageSession::new("webchat", "conversation-b");

    sink.send(&session_a, MessageChain::plain("alpha"))
        .await
        .expect("first message should record");
    sink.send(&session_b, MessageChain::plain("beta"))
        .await
        .expect("second message should record");

    let filtered = platform
        .sent_messages_for_conversation("conversation-a")
        .await;

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].session.conversation_id, "conversation-a");
    assert_eq!(filtered[0].chain.plain_text(), "alpha");
}

#[tokio::test]
async fn onebot_platform_submits_private_text_events() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = OneBotPlatform::with_identity("onebot", "OneBot", event_tx, sink);

    let event_id = platform
        .submit_private_text("user-1", "hello onebot")
        .await
        .expect("onebot private input should submit event");

    let event = event_rx.recv().await.expect("event should be queued");
    assert_eq!(event.id, event_id);
    assert_eq!(event.platform_id, "onebot");
    assert_eq!(event.platform_name, "OneBot");
    assert_eq!(event.sender.id, "user-1");
    assert!(event.session.is_direct());
    assert_eq!(event.session.conversation_id, "private:user-1");
    assert_eq!(event.message.plain_text(), "hello onebot");
}

#[tokio::test]
async fn onebot_platform_submits_group_text_events() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = OneBotPlatform::with_identity("onebot", "OneBot", event_tx, sink);

    let event_id = platform
        .submit_group_text("group-1", "user-1", "hello group")
        .await
        .expect("onebot group input should submit event");

    let event = event_rx.recv().await.expect("event should be queued");
    assert_eq!(event.id, event_id);
    assert_eq!(event.platform_id, "onebot");
    assert_eq!(event.sender.id, "user-1");
    assert!(event.session.is_group());
    assert_eq!(event.session.conversation_id, "group:group-1");
    assert_eq!(event.message.plain_text(), "hello group");
}

#[tokio::test]
async fn onebot_platform_rejects_empty_message_chains() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(RecordingSink::default());
    let platform = OneBotPlatform::with_identity("onebot", "OneBot", event_tx, sink);

    let result = platform
        .submit_private_chain("user-1", MessageChain::default())
        .await;

    assert!(matches!(result, Err(AstrbotError::EmptyMessage)));
    assert!(event_rx.try_recv().is_err());
}

#[tokio::test]
async fn console_platform_parses_input_into_events() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    let sink = Arc::new(ConsoleSink::default());
    let platform = ConsolePlatform::with_identity("console", "Console", event_tx, sink);

    assert!(
        platform
            .handle_line("alice: hello from console")
            .await
            .expect("console line should be handled")
    );

    let event = event_rx.recv().await.expect("event should be queued");
    assert_eq!(event.platform_id, "console");
    assert_eq!(event.sender.id, "alice");
    assert_eq!(event.session.conversation_id, "console");
    assert_eq!(event.message.plain_text(), "hello from console");
}

#[tokio::test]
async fn console_sink_records_sent_messages() {
    let sink = ConsoleSink::default();
    let session = MessageSession::new("console", "console");

    sink.send(&session, MessageChain::plain("response"))
        .await
        .expect("console sink should record output");

    let sent = sink.messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "response");
}

#[tokio::test]
async fn recording_sink_records_streaming_messages_separately() {
    let sink = RecordingSink::default();
    let session = MessageSession::new("mock", "conversation-1");

    sink.send_streaming(
        &session,
        MessageStream::new(vec![MessageChain::plain("one"), MessageChain::plain("two")]),
    )
    .await
    .expect("streaming message should record");

    assert!(sink.messages().await.is_empty());
    let streamed = sink.streaming_messages().await;
    assert_eq!(streamed.len(), 1);
    assert_eq!(streamed[0].session, session);
    assert_eq!(streamed[0].stream.chunks()[0].plain_text(), "one");
    assert_eq!(streamed[0].stream.chunks()[1].plain_text(), "two");
}

#[test]
fn manager_skips_disabled_platforms() {
    let registry = PlatformRegistry::with_builtin_platforms();
    let (event_tx, _event_rx) = mpsc::channel(1);
    let manager = PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig::mock("disabled").disabled()],
        PlatformBuildContext::new(event_tx),
    )
    .expect("disabled platforms should be skipped");

    assert_eq!(manager.platform_count(), 0);
}

#[tokio::test]
async fn manager_run_all_executes_mock_adapters() {
    let registry = PlatformRegistry::with_builtin_platforms();
    let (event_tx, _event_rx) = mpsc::channel(1);
    let manager = PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig::mock("test-mock")],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build");

    manager
        .run_all()
        .await
        .expect("mock adapter run should finish");
}

#[tokio::test]
async fn manager_run_all_and_terminate_executes_onebot_transport() {
    let registry = PlatformRegistry::with_builtin_platforms();
    let (event_tx, _event_rx) = mpsc::channel(1);
    let manager = PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig::onebot("onebot")],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build");

    manager
        .run_all()
        .await
        .expect("onebot transport run should finish");
    manager
        .terminate()
        .await
        .expect("onebot transport terminate should finish");
}

#[tokio::test]
async fn manager_terminates_configured_platforms() {
    let terminate_count = Arc::new(AtomicU64::new(0));
    let adapter_count = terminate_count.clone();
    let mut registry = PlatformRegistry::new();
    registry
        .register_platform("terminating", move |config, _ctx| {
            Ok(BuiltPlatform::new(Arc::new(TerminatingPlatform {
                id: config.id.clone(),
                terminate_count: adapter_count.clone(),
            })))
        })
        .expect("custom platform should register");
    let (event_tx, _event_rx) = mpsc::channel(1);
    let manager = PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig {
            id: "platform-1".to_string(),
            platform_type: "terminating".to_string(),
            enabled: true,
            name: None,
        }],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build");

    manager.terminate().await.expect("manager should terminate");

    assert_eq!(terminate_count.load(Ordering::SeqCst), 1);
}

#[cfg(test)]
struct TerminatingPlatform {
    id: String,
    terminate_count: Arc<AtomicU64>,
}

#[cfg(test)]
#[async_trait]
impl PlatformAdapter for TerminatingPlatform {
    async fn run(&self) -> Result<()> {
        Ok(())
    }

    async fn terminate(&self) -> Result<()> {
        self.terminate_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Terminating Platform"
    }
}
