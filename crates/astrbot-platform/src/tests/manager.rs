use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use astrbot_core::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{
    BuiltPlatform, PlatformAdapter, PlatformBuildContext, PlatformConfig, PlatformManager,
    PlatformRegistry,
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

struct TerminatingPlatform {
    id: String,
    terminate_count: Arc<AtomicU64>,
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
