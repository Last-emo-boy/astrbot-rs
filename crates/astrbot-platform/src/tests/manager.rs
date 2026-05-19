use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use astrbot_core::{AstrbotError, Result};
use astrbot_observability::{
    ComponentKind, ComponentStatus, InMemoryStatusCollector, StatusEventSink,
};
use async_trait::async_trait;
use tokio::sync::{Notify, mpsc};

use crate::{
    BuiltPlatform, DINGTALK_PLATFORM_TYPE, LARK_PLATFORM_TYPE, LINE_PLATFORM_TYPE, PlatformAdapter,
    PlatformBuildContext, PlatformConfig, PlatformManager, PlatformRegistry, SLACK_PLATFORM_TYPE,
    TELEGRAM_PLATFORM_TYPE, WECOM_AI_BOT_PLATFORM_TYPE, WECOM_PLATFORM_TYPE,
};

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

#[test]
fn platform_config_debug_redacts_secrets() {
    let config = PlatformConfig::new("telegram", TELEGRAM_PLATFORM_TYPE)
        .with_option_string("telegram_api_base_url", "https://api.telegram.org/bot")
        .with_secret("telegram_token", "secret-token");

    let debug = format!("{config:?}");

    assert!(debug.contains("telegram_api_base_url"));
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("secret-token"));
}

#[test]
fn builtin_registry_validates_required_platform_payloads() {
    let registry = PlatformRegistry::with_builtin_platforms();
    let (event_tx, _event_rx) = mpsc::channel(1);

    let missing_onebot = match PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig::new("onebot", "onebot")],
        PlatformBuildContext::new(event_tx.clone()),
    ) {
        Ok(_) => panic!("onebot without host and port should fail"),
        Err(err) => err,
    };
    assert!(missing_onebot.to_string().contains("ws_reverse_host"));

    let missing_slack = match PlatformManager::from_configs(
        &registry,
        vec![
            PlatformConfig::new("slack", SLACK_PLATFORM_TYPE)
                .with_option_string("slack_connection_mode", "socket")
                .with_secret("bot_token", "xoxb-token"),
        ],
        PlatformBuildContext::new(event_tx.clone()),
    ) {
        Ok(_) => panic!("slack socket mode without app_token should fail"),
        Err(err) => err,
    };
    assert!(missing_slack.to_string().contains("app_token"));

    let manager = PlatformManager::from_configs(
        &registry,
        vec![
            PlatformConfig::new("telegram", TELEGRAM_PLATFORM_TYPE)
                .with_secret("telegram_token", "telegram-token"),
            PlatformConfig::new("slack", SLACK_PLATFORM_TYPE)
                .with_option_string("slack_connection_mode", "socket")
                .with_secret("bot_token", "xoxb-token")
                .with_secret("app_token", "xapp-token"),
            PlatformConfig::new("lark", LARK_PLATFORM_TYPE)
                .with_option_string("app_id", "cli_xxx")
                .with_secret("app_secret", "lark-secret"),
            PlatformConfig::new("line", LINE_PLATFORM_TYPE)
                .with_secret("channel_access_token", "line-token")
                .with_secret("channel_secret", "line-secret"),
            PlatformConfig::new("wecom", WECOM_PLATFORM_TYPE)
                .with_secret("corpid", "corp-id")
                .with_secret("secret", "corp-secret"),
            PlatformConfig::new("wecom-ai", WECOM_AI_BOT_PLATFORM_TYPE)
                .with_option_string("wecom_ai_bot_connection_mode", "long_connection")
                .with_option_u16("port", 6198)
                .with_secret("wecomaibot_ws_bot_id", "bot-id")
                .with_secret("wecomaibot_ws_secret", "bot-secret"),
            PlatformConfig::new("dingtalk", DINGTALK_PLATFORM_TYPE)
                .with_option_string("client_id", "ding-client")
                .with_secret("client_secret", "ding-secret"),
        ],
        PlatformBuildContext::new(event_tx),
    )
    .expect("valid wave1 platform payloads should build");

    assert_eq!(manager.platform_count(), 7);
    assert!(manager.adapter("telegram").is_some());
    assert!(manager.adapter("wecom-ai").is_some());
    assert!(manager.adapter("dingtalk").is_some());
    assert_eq!(manager.recording_sink_count(), 7);
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
        vec![
            PlatformConfig::new("onebot", "onebot")
                .with_option_string("ws_reverse_host", "127.0.0.1")
                .with_option_u16("ws_reverse_port", 0),
        ],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build");

    let tasks = manager.spawn_all();
    tokio::task::yield_now().await;
    manager
        .terminate()
        .await
        .expect("onebot transport terminate should finish");
    for task in tasks {
        task.await
            .expect("onebot task should join")
            .expect("onebot transport run should finish");
    }
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
        vec![PlatformConfig::new("platform-1", "terminating")],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build");

    manager.terminate().await.expect("manager should terminate");

    assert_eq!(terminate_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn manager_terminate_releases_blocking_adapter_before_join() {
    let terminate_count = Arc::new(AtomicU64::new(0));
    let run_completed = Arc::new(AtomicBool::new(false));
    let adapter = Arc::new(BlockingPlatform {
        id: "blocking-1".to_string(),
        terminate_count: terminate_count.clone(),
        run_completed: run_completed.clone(),
        shutdown: Notify::new(),
    });
    let adapter_for_factory = adapter.clone();
    let mut registry = PlatformRegistry::new();
    registry
        .register_platform("blocking", move |_config, _ctx| {
            Ok(BuiltPlatform::new(adapter_for_factory.clone()))
        })
        .expect("custom platform should register");
    let sink = Arc::new(InMemoryStatusCollector::new());
    let (event_tx, _event_rx) = mpsc::channel(1);
    let manager = PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig::new("blocking-1", "blocking")],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build")
    .with_status_sink(sink.clone() as Arc<dyn StatusEventSink>);

    let tasks = manager.spawn_all();
    tokio::task::yield_now().await;
    manager
        .terminate()
        .await
        .expect("manager should terminate blocking adapter");
    for task in tasks {
        task.await
            .expect("blocking task should join")
            .expect("blocking adapter should finish");
    }

    assert_eq!(terminate_count.load(Ordering::SeqCst), 1);
    assert!(run_completed.load(Ordering::SeqCst));
    let events = sink.events();
    assert!(events.iter().any(|event| {
        event.component == ComponentKind::Platform
            && event.component_id.as_deref() == Some("blocking-1")
            && event.status == ComponentStatus::Stopping
    }));
    assert!(events.iter().any(|event| {
        event.component == ComponentKind::Platform
            && event.component_id.as_deref() == Some("blocking-1")
            && event.status == ComponentStatus::Stopped
    }));
}

#[tokio::test]
async fn manager_run_all_propagates_platform_startup_failure() {
    let mut registry = PlatformRegistry::new();
    registry
        .register_platform("failing", |config, _ctx| {
            Ok(BuiltPlatform::new(Arc::new(FailingPlatform {
                id: config.id.clone(),
                panic_on_run: false,
            })))
        })
        .expect("custom platform should register");
    let sink = Arc::new(InMemoryStatusCollector::new());
    let (event_tx, _event_rx) = mpsc::channel(1);
    let manager = PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig::new("failing-1", "failing")],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build")
    .with_status_sink(sink.clone() as Arc<dyn StatusEventSink>);

    let err = manager
        .run_all()
        .await
        .expect_err("startup failure should propagate");

    assert!(err.to_string().contains("startup failure"));
    assert!(sink.events().iter().any(|event| {
        event.component == ComponentKind::Platform
            && event.component_id.as_deref() == Some("failing-1")
            && event.status == ComponentStatus::Failed
    }));
}

#[tokio::test]
async fn manager_run_all_maps_platform_task_panic_to_join_error() {
    let mut registry = PlatformRegistry::new();
    registry
        .register_platform("panic", |config, _ctx| {
            Ok(BuiltPlatform::new(Arc::new(FailingPlatform {
                id: config.id.clone(),
                panic_on_run: true,
            })))
        })
        .expect("custom platform should register");
    let (event_tx, _event_rx) = mpsc::channel(1);
    let manager = PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig::new("panic-1", "panic")],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build");

    let err = manager
        .run_all()
        .await
        .expect_err("panic should map to join error");

    assert!(err.to_string().contains("platform task join failed"));
}

struct TerminatingPlatform {
    id: String,
    terminate_count: Arc<AtomicU64>,
}

struct BlockingPlatform {
    id: String,
    terminate_count: Arc<AtomicU64>,
    run_completed: Arc<AtomicBool>,
    shutdown: Notify,
}

#[async_trait]
impl PlatformAdapter for BlockingPlatform {
    async fn run(&self) -> Result<()> {
        self.shutdown.notified().await;
        self.run_completed.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn terminate(&self) -> Result<()> {
        self.terminate_count.fetch_add(1, Ordering::SeqCst);
        self.shutdown.notify_waiters();
        Ok(())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Blocking Platform"
    }
}

struct FailingPlatform {
    id: String,
    panic_on_run: bool,
}

#[async_trait]
impl PlatformAdapter for FailingPlatform {
    async fn run(&self) -> Result<()> {
        assert!(!self.panic_on_run, "platform panic");
        Err(AstrbotError::Platform("startup failure".to_string()))
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Failing Platform"
    }
}

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
