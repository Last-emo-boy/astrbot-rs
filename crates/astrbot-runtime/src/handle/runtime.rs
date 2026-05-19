use std::sync::Arc;

use astrbot_core::event::{EventLogRecord, EventLogger};
use astrbot_core::{AstrbotError, EventBus, MessageEvent, Result};
use astrbot_metrics::{MetricEvent, MetricSink, NoopMetricSink};
use astrbot_observability::{
    ComponentKind, ComponentStatus, InMemoryLogBuffer, LogEntry, LogLevel, LogSource,
    NoopStatusEventSink, StatusEvent, StatusEventSink,
};
use astrbot_pipeline::{
    ContentSafetyConfig, DefaultPipelineBuilder, InMemoryProviderPreferencePort, PipelineContext,
    PipelineScheduler, ProviderFallbackConfig, RateLimitConfig, ResultDecorateConfig,
    WakeCheckConfig, WhitelistPolicyConfig,
};
use astrbot_platform::{PlatformManager, SentMessage};
use astrbot_plugin::PluginRegistry;
use astrbot_provider::ProviderManager;
use astrbot_render::{
    LocalMarkdownRenderer, NetworkT2iEndpointCatalog, NetworkT2iRenderer, ReqwestNetworkT2iClient,
    TemplateCatalog, TemplateRenderer,
};
use astrbot_storage::TempArtifactRoot;
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
    event_sender: mpsc::Sender<MessageEvent>,
    platform_manager: PlatformManager,
    provider_manager: ProviderManager,
    pub(super) provider_preference: Arc<InMemoryProviderPreferencePort>,
    plugin_registry: Arc<PluginRegistry>,
    scheduler: Arc<PipelineScheduler>,
    status_sink: Arc<dyn StatusEventSink>,
    pub(super) metric_sink: Arc<dyn MetricSink>,
}

pub struct RuntimeHandle {
    pub(super) tasks: RuntimeTaskSet,
    platform_manager: PlatformManager,
    provider_manager: ProviderManager,
    pub(super) provider_preference: Arc<InMemoryProviderPreferencePort>,
    plugin_registry: Arc<PluginRegistry>,
    pub(super) status_sink: Arc<dyn StatusEventSink>,
    pub(super) metric_sink: Arc<dyn MetricSink>,
}

impl AstrbotRuntime {
    pub fn initialize(config: RuntimeConfig) -> Result<Self> {
        let queue_capacity = config.event_queue_capacity.max(1);
        let (event_tx, event_rx) = mpsc::channel(queue_capacity);
        let platform_manager = build_platform_manager(&config, event_tx.clone())?;
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
        let provider_fallback = ProviderFallbackConfig::from(config.provider_fallback.clone())
            .with_provider_wake_prefixes(
                config.provider_fallback.wake_prefix.clone(),
                config.wake_check.wake_prefixes.clone(),
            );
        let result_decorate = ResultDecorateConfig::from(config.result_decorate.clone());

        let path_layout = config.paths.resolve();
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
        let context = if provider_manager.text_to_speech_provider_count() > 0 {
            context.with_text_to_speech_provider(Arc::new(provider_manager.clone()))
        } else {
            context
        };
        let context = if config.result_decorate.t2i_enabled {
            context.with_t2i_renderer(default_t2i_renderer(&config.result_decorate, &path_layout))
        } else {
            context
        };
        let scheduler = DefaultPipelineBuilder::new()?.build(context)?;
        let scheduler = Arc::new(scheduler);
        let event_bus = EventBus::new(event_rx, scheduler.clone());

        Ok(Self {
            config,
            event_bus,
            event_sender: event_tx,
            platform_manager,
            provider_manager,
            provider_preference,
            plugin_registry,
            scheduler,
            status_sink: Arc::new(NoopStatusEventSink),
            metric_sink: Arc::new(NoopMetricSink),
        })
    }

    pub fn with_status_sink(mut self, status_sink: Arc<dyn StatusEventSink>) -> Self {
        self.platform_manager = self.platform_manager.with_status_sink(status_sink.clone());
        self.provider_manager = self.provider_manager.with_status_sink(status_sink.clone());
        self.status_sink = status_sink;
        self
    }

    pub fn with_metric_sink(mut self, metric_sink: Arc<dyn MetricSink>) -> Self {
        self.metric_sink = metric_sink;
        self
    }

    pub fn with_log_buffer(mut self, logs: Arc<InMemoryLogBuffer>) -> Self {
        self.event_bus = self
            .event_bus
            .with_logger(Arc::new(RuntimeEventLogSink::new(logs)));
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

    pub fn event_sender(&self) -> mpsc::Sender<MessageEvent> {
        self.event_sender.clone()
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
            event_sender: _,
            platform_manager,
            provider_manager,
            provider_preference,
            plugin_registry,
            status_sink,
            metric_sink,
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
            metric_sink,
        }
    }

    pub async fn sent_messages(&self) -> Vec<SentMessage> {
        self.sent_messages_for(DEFAULT_MOCK_PLATFORM_ID).await
    }

    pub async fn sent_messages_for(&self, platform_id: &str) -> Vec<SentMessage> {
        testing::sent_messages_for(&self.platform_manager, platform_id).await
    }
}

fn default_t2i_renderer(
    config: &crate::policy_config::RuntimeResultDecorateConfig,
    path_layout: &crate::path_config::RuntimePathLayout,
) -> Arc<dyn astrbot_render::T2iRenderer> {
    match config.t2i_strategy.trim() {
        "template" | "html" | "local_template" => {
            return Arc::new(TemplateRenderer::new(
                TemplateCatalog::new(&path_layout.t2i_template_dir),
                &path_layout.generated_media_dir,
            ));
        }
        "remote" | "network" | "network_only" => {
            let catalog = config
                .t2i_endpoint
                .as_deref()
                .map(NetworkT2iEndpointCatalog::new)
                .transpose()
                .unwrap_or(None)
                .unwrap_or_else(NetworkT2iEndpointCatalog::default_official);
            return Arc::new(NetworkT2iRenderer::new(
                catalog,
                TemplateCatalog::new(&path_layout.t2i_template_dir),
                Arc::new(ReqwestNetworkT2iClient::default()),
            ));
        }
        _ => {}
    }

    Arc::new(LocalMarkdownRenderer::new(TempArtifactRoot::new(
        &path_layout.temp_dir,
    )))
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
        .await?;
        self.metric_sink
            .record(MetricEvent::platform_message(
                current_unix_timestamp().to_string(),
                platform_id,
                platform_id,
                1,
            ))
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
            metric_sink: _,
            ..
        } = self;

        status_sink.emit(StatusEvent::new(
            ComponentKind::Runtime,
            ComponentStatus::Stopping,
        ));

        let mut first_error = None;
        if let Err(err) = platform_manager.terminate().await {
            remember_stop_error(&mut first_error, err);
        }
        if let Err(err) = tasks.stop().await {
            remember_stop_error(&mut first_error, err);
            status_sink.emit(StatusEvent::new(
                ComponentKind::Task,
                ComponentStatus::Failed,
            ));
        } else {
            status_sink.emit(StatusEvent::new(
                ComponentKind::Task,
                ComponentStatus::Stopped,
            ));
        }
        if let Err(err) = plugin_registry.terminate().await {
            remember_stop_error(&mut first_error, err);
        }
        if let Err(err) = provider_manager.terminate().await {
            remember_stop_error(&mut first_error, err);
        }

        if let Some(err) = first_error {
            status_sink.emit(StatusEvent::new(
                ComponentKind::Runtime,
                ComponentStatus::Failed,
            ));
            Err(err)
        } else {
            status_sink.emit(StatusEvent::new(
                ComponentKind::Runtime,
                ComponentStatus::Stopped,
            ));
            Ok(())
        }
    }
}

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn remember_stop_error(first_error: &mut Option<AstrbotError>, err: AstrbotError) {
    if first_error.is_none() {
        *first_error = Some(err);
    }
}

struct RuntimeEventLogSink {
    logs: Arc<InMemoryLogBuffer>,
}

impl RuntimeEventLogSink {
    fn new(logs: Arc<InMemoryLogBuffer>) -> Self {
        Self { logs }
    }
}

impl EventLogger for RuntimeEventLogSink {
    fn log_event(&self, record: &EventLogRecord) {
        let logs = self.logs.clone();
        let entry = LogEntry::new(LogLevel::Info, LogSource::Runtime, record.display_line())
            .with_target(record.event_id.clone());
        tokio::spawn(async move {
            logs.push(entry).await;
        });
    }
}
