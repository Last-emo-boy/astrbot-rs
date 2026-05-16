use astrbot_platform::WEBCHAT_PLATFORM_TYPE;

use crate::defaults::DEFAULT_MOCK_RESPONSE;
use crate::{AstrbotRuntime, RuntimeConfig, RuntimePlatformConfig, RuntimeWakeCheckConfig};

#[tokio::test]
async fn runtime_builds_platforms_from_config() {
    let config = RuntimeConfig {
        platforms: vec![RuntimePlatformConfig::mock("test-mock").with_name("Test Mock")],
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text_on("test-mock", "event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages_for("test-mock").await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].session.platform_id, "test-mock");
    assert_eq!(sent[0].chain.plain_text(), DEFAULT_MOCK_RESPONSE);
    assert_eq!(runtime.platform_manager().platform_count(), 1);
}

#[test]
fn runtime_builds_console_platform_from_config() {
    let config = RuntimeConfig {
        platforms: vec![RuntimePlatformConfig::console("console").with_name("Console")],
        ..RuntimeConfig::default()
    };
    let runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    assert_eq!(runtime.platform_manager().platform_count(), 1);
    let adapter = runtime
        .platform_manager()
        .adapter("console")
        .expect("console adapter should exist");
    assert_eq!(adapter.id(), "console");
    assert_eq!(adapter.name(), "Console");
    assert!(
        runtime
            .platform_manager()
            .recording_sink("console")
            .is_some()
    );
}

#[tokio::test]
async fn runtime_webchat_platform_submits_events_to_pipeline() {
    let config = RuntimeConfig {
        platforms: vec![RuntimePlatformConfig::webchat("webchat").with_name("WebChat")],
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");
    let webchat = runtime
        .platform_manager()
        .webchat_platform("webchat")
        .expect("webchat platform should exist");

    webchat
        .submit_text("conversation-1", "user-1", "hello")
        .await
        .expect("webchat input should submit event");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages_for("webchat").await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].session.conversation_id, "conversation-1");
    assert_eq!(sent[0].chain.plain_text(), DEFAULT_MOCK_RESPONSE);
}

#[tokio::test]
async fn runtime_onebot_platform_submits_events_to_pipeline() {
    let config = RuntimeConfig {
        platforms: vec![RuntimePlatformConfig::onebot("onebot").with_name("OneBot")],
        wake_check: RuntimeWakeCheckConfig {
            wake_prefixes: vec!["/".to_string()],
            ..RuntimeWakeCheckConfig::default()
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");
    let onebot = runtime
        .platform_manager()
        .onebot_platform("onebot")
        .expect("onebot platform should exist");

    onebot
        .submit_private_text("user-1", "hello")
        .await
        .expect("onebot private input should submit event");
    runtime
        .run_once()
        .await
        .expect("private event should process");

    onebot
        .submit_group_text("group-1", "user-2", "/hello group")
        .await
        .expect("onebot group input should submit event");
    runtime
        .run_once()
        .await
        .expect("group event should process");

    let sent = runtime.sent_messages_for("onebot").await;
    assert_eq!(sent.len(), 2);
    assert!(sent[0].session.is_direct());
    assert_eq!(sent[0].session.conversation_id, "private:user-1");
    assert_eq!(sent[0].chain.plain_text(), DEFAULT_MOCK_RESPONSE);
    assert!(sent[1].session.is_group());
    assert_eq!(sent[1].session.conversation_id, "group:group-1");
    assert_eq!(sent[1].chain.plain_text(), DEFAULT_MOCK_RESPONSE);
}

#[test]
fn webchat_platform_type_constant_remains_runtime_default() {
    assert_eq!(WEBCHAT_PLATFORM_TYPE, "webchat");
}
