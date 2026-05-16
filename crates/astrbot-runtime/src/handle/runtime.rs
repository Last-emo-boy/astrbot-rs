use std::sync::Arc;

use astrbot_core::{EventBus, Result};
use astrbot_observability::{
    ComponentKind, ComponentStatus, NoopStatusEventSink, StatusEvent, StatusEventSink,
};
use astrbot_pipeline::{
    ContentSafetyConfig, DefaultPipelineBuilder, InMemoryProviderPreferencePort, PipelineContext,
    PipelineScheduler, ProviderFallbackConfig, RateLimitConfig, ResultDecorateConfig,
    WakeCheckConfig, WhitelistPolicyConfig,
};
use astrbot_platform::{PlatformManager, SentMessage};
use astrbot_plugin::PluginRegistry;
use astrbot_provider::ProviderManager;
use tokio::sync::mpsc;

use crate::RuntimeConfig;
use crate::assembly::{build_platform_manager, build_plugin_registry, build_provider_manager};
use crate::defaults::DEFAULT_MOCK_PLATFORM_ID;
use crate::ports::ConfiguredSessionStatusPort;

use super::supervisor::RuntimeTaskSet;
use super::testing;

pub struct AstrbotRuntime {
    config: RuntimeConfig,
    event_bus: EventBus,
    platform_manager: PlatformManager,
    provider_manager: ProviderManager,
    pub(super) provider_preference: Arc<InMemoryProviderPreferencePort>,
    plugin_registry: Arc<PluginRegistry>,
    scheduler: Arc<PipelineScheduler>,
    status_sink: Arc<dyn StatusEventSink>,
}

pub struct RuntimeHandle {
    pub(super) tasks: RuntimeTaskSet,
    platform_manager: PlatformManager,
    provider_manager: ProviderManager,
    pub(super) provider_preference: Arc<InMemoryProviderPreferencePort>,
    plugin_registry: Arc<PluginRegistry>,
    pub(super) status_sink: Arc<dyn StatusEventSink>,
}

impl AstrbotRuntime {
    pub fn initialize(config: RuntimeConfig) -> Result<Self> {
        let queue_capacity = config.event_queue_capacity.max(1);
        let (event_tx, event_rx) = mpsc::channel(queue_capacity);
        let platform_manager = build_platform_manager(&config, event_tx)?;
        let provider_manager = build_provider_manager(&config)?;
        let provider_preference = Arc::new(InMemoryProviderPreferencePort::new());
        let plugin_registry = build_plugin_registry(&config);
        let wake_check = WakeCheckConfig::from(config.wake_check.clone());
        let whitelist_policy = WhitelistPolicyConfig::from(config.whitelist_policy.clone());
        let session_status = Arc::new(ConfiguredSessionStatusPort::new(
            config.session_status.clone(),
        ));
        let rate_limit = RateLimitConfig::from(config.rate_limit.clone());
        let content_safety = ContentSafetyConfig::from(config.content_safety.clone());
        let provider_fallback = ProviderFallbackConfig::from(config.provider_fallback.clone());
        let result_decorate = ResultDecorateConfig::from(config.result_decorate.clone());

        let context = if provider_manager.chat_provider_count() > 0 {
            PipelineContext::with_chat_provider(Arc::new(provider_manager.clone()))
        } else {
            PipelineContext::new()
        }
        .with_wake_check(wake_check)
        .with_whitelist_policy(whitelist_policy)
        .with_session_status_port(session_status)
        .with_provider_preference_port(provider_preference.clone())
        .with_rate_limit(rate_limit)
        .with_content_safety(content_safety)
        .with_provider_fallback(provider_fallback)
        .with_result_decorate(result_decorate)
        .with_plugin_registry(plugin_registry.clone());
        let scheduler = DefaultPipelineBuilder::new()?.build(context)?;
        let scheduler = Arc::new(scheduler);
        let event_bus = EventBus::new(event_rx, scheduler.clone());

        Ok(Self {
            config,
            event_bus,
            platform_manager,
            provider_manager,
            provider_preference,
            plugin_registry,
            scheduler,
            status_sink: Arc::new(NoopStatusEventSink),
        })
    }

    pub fn with_status_sink(mut self, status_sink: Arc<dyn StatusEventSink>) -> Self {
        self.platform_manager = self.platform_manager.with_status_sink(status_sink.clone());
        self.provider_manager = self.provider_manager.with_status_sink(status_sink.clone());
        self.status_sink = status_sink;
        self
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn provider_manager(&self) -> &ProviderManager {
        &self.provider_manager
    }

    pub fn provider_preference(&self) -> Arc<InMemoryProviderPreferencePort> {
        self.provider_preference.clone()
    }

    pub fn platform_manager(&self) -> &PlatformManager {
        &self.platform_manager
    }

    pub fn plugin_registry(&self) -> Arc<PluginRegistry> {
        self.plugin_registry.clone()
    }

    pub fn scheduler(&self) -> Arc<PipelineScheduler> {
        self.scheduler.clone()
    }

    pub async fn emit_mock_text(
        &self,
        event_id: impl Into<String>,
        conversation_id: impl Into<String>,
        sender_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<()> {
        self.emit_mock_text_on(
            DEFAULT_MOCK_PLATFORM_ID,
            event_id,
            conversation_id,
            sender_id,
            text,
        )
        .await
    }

    pub async fn emit_mock_text_on(
        &self,
        platform_id: &str,
        event_id: impl Into<String>,
        conversation_id: impl Into<String>,
        sender_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<()> {
        testing::emit_mock_text_on(
            &self.platform_manager,
            platform_id,
            event_id,
            conversation_id,
            sender_id,
            text,
        )
        .await
    }

    pub async fn run_once(&mut self) -> Result<bool> {
        self.event_bus.run_once().await
    }

    pub async fn run(self) -> Result<()> {
        self.event_bus.run().await
    }

    pub fn start(self) -> RuntimeHandle {
        let AstrbotRuntime {
            event_bus,
            platform_manager,
            provider_manager,
            provider_preference,
            plugin_registry,
            status_sink,
            ..
        } = self;
        status_sink.emit(StatusEvent::new(
            ComponentKind::Runtime,
            ComponentStatus::Starting,
        ));
        let tasks = RuntimeTaskSet::spawn(event_bus, &platform_manager);
        status_sink.emit(StatusEvent::new(
            ComponentKind::Runtime,
            ComponentStatus::Running,
        ));

        RuntimeHandle {
            tasks,
            platform_manager,
            provider_manager,
            provider_preference,
            plugin_registry,
            status_sink,
        }
    }

    pub async fn sent_messages(&self) -> Vec<SentMessage> {
        self.sent_messages_for(DEFAULT_MOCK_PLATFORM_ID).await
    }

    pub async fn sent_messages_for(&self, platform_id: &str) -> Vec<SentMessage> {
        testing::sent_messages_for(&self.platform_manager, platform_id).await
    }
}

impl RuntimeHandle {
    pub fn platform_manager(&self) -> &PlatformManager {
        &self.platform_manager
    }

    pub fn provider_preference(&self) -> Arc<InMemoryProviderPreferencePort> {
        self.provider_preference.clone()
    }

    pub async fn emit_mock_text(
        &self,
        event_id: impl Into<String>,
        conversation_id: impl Into<String>,
        sender_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<()> {
        self.emit_mock_text_on(
            DEFAULT_MOCK_PLATFORM_ID,
            event_id,
            conversation_id,
            sender_id,
            text,
        )
        .await
    }

    pub async fn emit_mock_text_on(
        &self,
        platform_id: &str,
        event_id: impl Into<String>,
        conversation_id: impl Into<String>,
        sender_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<()> {
        testing::emit_mock_text_on(
            &self.platform_manager,
            platform_id,
            event_id,
            conversation_id,
            sender_id,
            text,
        )
        .await
    }

    pub async fn sent_messages(&self) -> Vec<SentMessage> {
        self.sent_messages_for(DEFAULT_MOCK_PLATFORM_ID).await
    }

    pub async fn sent_messages_for(&self, platform_id: &str) -> Vec<SentMessage> {
        testing::sent_messages_for(&self.platform_manager, platform_id).await
    }

    pub async fn stop(self) -> Result<()> {
        let RuntimeHandle {
            tasks,
            platform_manager,
            provider_manager,
            plugin_registry,
            status_sink,
            ..
        } = self;

        status_sink.emit(StatusEvent::new(
            ComponentKind::Runtime,
            ComponentStatus::Stopping,
        ));
        tasks.stop().await?;
        status_sink.emit(StatusEvent::new(
            ComponentKind::Task,
            ComponentStatus::Stopped,
        ));
        plugin_registry.terminate().await?;
        provider_manager.terminate().await?;
        platform_manager.terminate().await?;
        status_sink.emit(StatusEvent::new(
            ComponentKind::Runtime,
            ComponentStatus::Stopped,
        ));
        Ok(())
    }
}
