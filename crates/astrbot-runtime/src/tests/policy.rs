use crate::{
    AstrbotRuntime, RuntimeConfig, RuntimeContentSafetyConfig, RuntimeKeywordContentSafetyConfig,
    RuntimeProviderFallbackConfig, RuntimeResultDecorateConfig, RuntimeSessionStatusConfig,
};

#[tokio::test]
async fn runtime_session_status_policy_stops_disabled_sessions() {
    let config = RuntimeConfig {
        session_status: RuntimeSessionStatusConfig {
            disabled_sessions: vec!["conversation-1".to_string()],
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    assert!(runtime.sent_messages().await.is_empty());
}

#[tokio::test]
async fn runtime_provider_fallback_can_be_disabled() {
    let config = RuntimeConfig {
        provider_fallback: RuntimeProviderFallbackConfig {
            enabled: false,
            ..RuntimeProviderFallbackConfig::default()
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    assert!(runtime.sent_messages().await.is_empty());
    assert_eq!(runtime.provider_manager().chat_provider_count(), 1);
}

#[tokio::test]
async fn runtime_result_decorate_adds_reply_prefix() {
    let config = RuntimeConfig {
        result_decorate: RuntimeResultDecorateConfig {
            reply_prefix: Some("[bot] ".to_string()),
            only_llm_result: true,
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "[bot] hello from astrbot-rs");
}

#[tokio::test]
async fn runtime_content_safety_policy_blocks_configured_keywords() {
    let config = RuntimeConfig {
        content_safety: RuntimeContentSafetyConfig {
            rejection_message: Some("blocked".to_string()),
            internal_keywords: RuntimeKeywordContentSafetyConfig {
                enabled: true,
                extra_keywords: vec!["unsafe".to_string()],
            },
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "unsafe request")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "blocked");
}
