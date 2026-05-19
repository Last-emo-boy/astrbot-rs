use crate::defaults::DEFAULT_MOCK_RESPONSE;
use crate::{AstrbotRuntime, RuntimeChatProviderConfig, RuntimeCommandPluginConfig, RuntimeConfig};

#[tokio::test]
async fn runtime_wires_mock_platform_provider_and_pipeline() {
    let mut runtime =
        AstrbotRuntime::initialize(RuntimeConfig::default()).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    assert!(runtime.run_once().await.expect("event should process"));

    let sent = runtime.sent_messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), DEFAULT_MOCK_RESPONSE);
    assert_eq!(runtime.platform_manager().platform_count(), 1);
    assert_eq!(runtime.provider_manager().chat_provider_count(), 1);
    assert_eq!(runtime.scheduler().stage_count(), 9);
    assert_eq!(
        runtime.scheduler().stage_names(),
        vec![
            "wake".to_string(),
            "whitelist".to_string(),
            "session_status".to_string(),
            "rate_limit".to_string(),
            "content_safety".to_string(),
            "preprocess".to_string(),
            "process".to_string(),
            "result_decorate".to_string(),
            "respond".to_string(),
        ]
    );
}

#[tokio::test]
async fn configured_command_plugin_runs_before_provider() {
    let config = RuntimeConfig {
        command_plugins: vec![RuntimeCommandPluginConfig {
            plugin_name: "builtin".to_string(),
            handler_name: "ping".to_string(),
            command: "ping".to_string(),
            response: "pong".to_string(),
            priority: 10,
            enabled: true,
            permission: Default::default(),
        }],
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "/ping")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "pong");
    assert!(runtime.plugin_registry().handler_count() > 1);
}

#[tokio::test]
async fn command_plugin_can_run_without_chat_provider() {
    let config = RuntimeConfig {
        chat_providers: Vec::new(),
        command_plugins: vec![RuntimeCommandPluginConfig {
            plugin_name: "builtin".to_string(),
            handler_name: "ping".to_string(),
            command: "ping".to_string(),
            response: "pong".to_string(),
            priority: 10,
            enabled: true,
            permission: Default::default(),
        }],
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "/ping")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "pong");
    assert_eq!(runtime.provider_manager().chat_provider_count(), 0);
}

#[tokio::test]
async fn runtime_registers_builtin_command_descriptors() {
    let runtime =
        AstrbotRuntime::initialize(RuntimeConfig::default()).expect("runtime should initialize");
    let registry = runtime.plugin_registry();
    let commands = registry.command_descriptors();
    let names = commands
        .iter()
        .map(|command| command.effective_command())
        .collect::<Vec<_>>();

    assert!(names.contains(&"help".to_string()));
    assert!(names.contains(&"plugin ls".to_string()));
    assert!(names.contains(&"alter_cmd".to_string()));
    assert!(
        commands
            .iter()
            .any(|command| command.effective_aliases() == vec!["alter"])
    );
    assert!(registry.command_conflicts().is_empty());
}

#[tokio::test]
async fn runtime_builtin_commands_handle_messages_before_provider() {
    let mut runtime =
        AstrbotRuntime::initialize(RuntimeConfig::default()).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "/sid")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");
    runtime
        .emit_mock_text("event-2", "conversation-1", "user-1", "/plugin ls")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");
    runtime
        .emit_mock_text("event-3", "conversation-1", "user-1", "/llm")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    assert_eq!(sent.len(), 3);
    assert!(
        sent[0]
            .chain
            .plain_text()
            .contains("Session ID: conversation-1")
    );
    assert_eq!(sent[1].chain.plain_text(), "已安装插件: builtin_commands");
    assert_eq!(sent[2].chain.plain_text(), DEFAULT_MOCK_RESPONSE);
}

#[tokio::test]
async fn runtime_default_pipeline_applies_session_provider_preference() {
    let config = RuntimeConfig {
        default_chat_provider_id: "default-provider".to_string(),
        chat_providers: vec![
            RuntimeChatProviderConfig::mock("default-provider", "default-response"),
            RuntimeChatProviderConfig::mock("preferred-provider", "preferred-response"),
        ],
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");
    runtime
        .provider_preference()
        .set_preferred_chat_provider("conversation-1", "preferred-provider")
        .await
        .expect("provider preference should be stored");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "preferred-response");
}
