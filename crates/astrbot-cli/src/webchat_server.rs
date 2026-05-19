use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use astrbot_agent::{SubagentConfig, SubagentConfigSource};
use astrbot_core::Result as AstrbotResult;
use astrbot_core::{AstrbotError, MessageEvent};
use astrbot_cron::{
    ActiveAgentCronPayload, CronJob, CronJobSchedule, CronScheduler, DueCronScheduleDriver,
    ProactiveAgentWakeService, SqliteCronJobRepository,
};
use astrbot_maintenance::SqliteMaintenanceOperationStore;
use astrbot_metrics::{MetricEvent, MetricTtsStats, UsageRecord};
use astrbot_observability::{InMemoryLogBuffer, LogEntry, LogLevel, LogSource, TraceEvent};
use astrbot_persona::{PersonaFolder, PersonaManager, PersonaProfile, SqlitePersonaRepository};
use astrbot_platform::WebChatPlatform;
use astrbot_plugin::{
    PluginCompatibility, PluginInstallSource, PluginLifecycleState, PluginLoadSource,
    PluginManifest, PluginMarketEntry, PluginPackageDescriptor,
};
use astrbot_runtime::{
    AstrbotRuntime, DashboardAssetPolicy, RuntimeConfigService, RuntimeWebChatServerConfig,
    runtime_internal_tool_catalog,
};
use astrbot_skill::{SkillCatalog, SkillDescriptor, SkillSandboxCache, SkillSandboxEntry};
use astrbot_storage::{
    BackupJobService, FilesystemBackupExporter, SqliteBackupImporter, SqliteBackupRepository,
    SqliteJsonStore,
};
use astrbot_web::{
    LocalMaintenanceExecutor, MaintenanceRestartExecutor, ManagementApiState, ManagementAuthState,
    ManagementBackupState, ManagementConfigApplyState, ManagementConfigRouteState,
    ManagementCronState, ManagementKnowledgeBaseState, ManagementMaintenanceState,
    ManagementMcpState, ManagementObservabilityState, ManagementPersonaState,
    ManagementPluginLifecycleState, ManagementPluginSeed, ManagementSkillState,
    ManagementSubagentConfig, ManagementSubagentState, ManagementToolState,
    PluginMarketManagementState, serve_dashboard_with_auth_and_shutdown,
};
use async_trait::async_trait;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{self, MissedTickBehavior};

const DASHBOARD_RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) async fn prepare_webchat_server(
    runtime: &AstrbotRuntime,
    config: &RuntimeWebChatServerConfig,
    config_path: &Path,
) -> Result<Option<PendingWebChatServer>, Box<dyn Error>> {
    prepare_webchat_server_with_config_apply(runtime, config, config_path, None, None).await
}

pub(crate) async fn prepare_webchat_server_with_config_apply(
    runtime: &AstrbotRuntime,
    config: &RuntimeWebChatServerConfig,
    config_path: &Path,
    config_apply: Option<ManagementConfigApplyState>,
    restart_executor: Option<Arc<dyn MaintenanceRestartExecutor>>,
) -> Result<Option<PendingWebChatServer>, Box<dyn Error>> {
    if !config.enabled {
        return Ok(None);
    }

    let webchat = runtime
        .platform_manager()
        .webchat_platform(&config.platform_id)
        .ok_or_else(|| {
            io::Error::other(format!(
                "webchat server platform {} is not configured",
                config.platform_id
            ))
        })?;
    let listener = TcpListener::bind((config.host.as_str(), config.port)).await?;
    let address = listener.local_addr()?;

    let layout = runtime.config().paths.resolve();
    let assets = DashboardAssetPolicy::from_layout(&layout)
        .with_explicit_webui_dir("dashboard")
        .select();
    let sqlite_path = layout.data_dir.join("main.sqlite");
    let knowledge_base_store = SqliteJsonStore::open(sqlite_path.clone())?;
    let persona_store = SqliteJsonStore::open(sqlite_path.clone())?;
    let subagent_store = SqliteJsonStore::open(sqlite_path.clone())?;
    let maintenance_store =
        SqliteMaintenanceOperationStore::new(SqliteJsonStore::open(sqlite_path.clone())?);
    let maintenance_executor = Arc::new(
        LocalMaintenanceExecutor::new(layout.root_dir.clone())
            .with_runtime_config_path(config_path.to_path_buf())
            .with_sqlite_path(sqlite_path.clone()),
    );
    let config_service = RuntimeConfigService::new(config_path);
    let config_routes = ManagementConfigRouteState::from_config_service(config_service.clone())?;
    let auth = ManagementAuthState::from_config_service(config_service.clone());
    let mut maintenance = ManagementMaintenanceState::new(DASHBOARD_RUNTIME_VERSION)
        .with_latest_version(DASHBOARD_RUNTIME_VERSION)
        .with_dashboard_version(DASHBOARD_RUNTIME_VERSION)
        .with_operation_store(Arc::new(maintenance_store))
        .with_release_executor(maintenance_executor.clone())
        .with_package_executor(maintenance_executor.clone())
        .with_migration_executor(maintenance_executor);
    if let Some(restart_executor) = restart_executor {
        maintenance = maintenance.with_restart_executor(restart_executor);
    }

    let mut management = ManagementApiState::from_managers(
        runtime.provider_manager(),
        runtime.platform_manager(),
        &runtime.plugin_registry(),
    )
    .with_sqlite_storage_path(sqlite_path.clone())?
    .with_file_allowed_root(layout.backups_dir.clone())
    .with_config_service(config_service)
    .with_config_routes(config_routes)
    .with_knowledge_base(ManagementKnowledgeBaseState::sqlite(
        runtime.provider_manager().clone(),
        knowledge_base_store,
    ))
    .with_tools(ManagementToolState::new(runtime_internal_tool_catalog()))
    .with_mcp(ManagementMcpState::default())
    .with_maintenance(maintenance)
    .with_plugin_lifecycle(default_plugin_lifecycle_state())
    .with_plugin_market(default_plugin_market_state())
    .with_skills(default_skill_state(&layout))
    .with_backup(default_backup_state(
        DASHBOARD_RUNTIME_VERSION,
        &layout,
        sqlite_path.clone(),
    ))
    .with_observability(default_observability_state(&layout).await?)
    .with_personas(default_persona_state(persona_store).await)
    .with_subagents(default_subagent_state(subagent_store)?);
    if let Some(config_apply) = config_apply {
        management = management.with_config_apply(config_apply);
    }
    let cron_store = SqliteJsonStore::open(sqlite_path)?;
    let (cron_state, cron_scheduler) = default_cron_state(runtime, &webchat, cron_store).await;
    let management = management.with_cron(cron_state);

    Ok(Some(PendingWebChatServer {
        listener,
        webchat,
        management,
        auth,
        assets,
        address,
        cron_scheduler: Some(cron_scheduler),
    }))
}

pub(crate) struct PendingWebChatServer {
    listener: TcpListener,
    webchat: Arc<WebChatPlatform>,
    management: ManagementApiState,
    auth: ManagementAuthState,
    assets: astrbot_runtime::DashboardAssetSelection,
    cron_scheduler: Option<Arc<CronScheduler>>,
    pub(crate) address: SocketAddr,
}

impl PendingWebChatServer {
    pub(crate) fn start(self) -> WebChatServerHandle {
        let address = self.address;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let cron_task = self.cron_scheduler.map(spawn_cron_tick_loop);
        let task = tokio::spawn(serve_dashboard_with_auth_and_shutdown(
            self.listener,
            self.webchat,
            self.management,
            self.auth,
            self.assets,
            async move {
                let _ = shutdown_rx.await;
            },
        ));

        WebChatServerHandle {
            address,
            shutdown_tx: Some(shutdown_tx),
            task,
            cron_task,
        }
    }
}

fn default_plugin_market_state() -> PluginMarketManagementState {
    PluginMarketManagementState::new(vec![
        PluginMarketEntry::new(
            "astrbot-web-tools",
            "AstrBot Web Tools",
            DASHBOARD_RUNTIME_VERSION,
        )
        .with_package(
            PluginPackageDescriptor::new(PluginInstallSource::archive(
                "https://example.invalid/astrbot-rs/plugins/astrbot-web-tools.zip",
            ))
            .with_cache_key("astrbot-web-tools"),
        )
        .with_compatibility(PluginCompatibility::compatible(DASHBOARD_RUNTIME_VERSION)),
        PluginMarketEntry::new(
            "astrbot-session-helper",
            "Session Helper",
            DASHBOARD_RUNTIME_VERSION,
        )
        .with_repo_url("https://example.invalid/astrbot-rs/plugins/session-helper.git")
        .with_compatibility(PluginCompatibility::compatible(DASHBOARD_RUNTIME_VERSION)),
    ])
}

fn default_plugin_lifecycle_state() -> ManagementPluginLifecycleState {
    ManagementPluginLifecycleState::new(vec![
        ManagementPluginSeed::new(
            PluginLoadSource::native_rust("astrbot-web-tools").with_root_dir("plugins/web-tools"),
            PluginManifest::new("AstrBot Web Tools", DASHBOARD_RUNTIME_VERSION)
                .with_description("Dashboard-facing plugin lifecycle fixture."),
            PluginLifecycleState::Active,
        ),
        ManagementPluginSeed::new(
            PluginLoadSource::python_compat("session-helper")
                .with_root_dir("plugins/session-helper"),
            PluginManifest::new("Session Helper", DASHBOARD_RUNTIME_VERSION)
                .with_description("Session helper plugin fixture."),
            PluginLifecycleState::Disabled,
        ),
    ])
}

fn default_skill_state(layout: &astrbot_runtime::RuntimePathLayout) -> ManagementSkillState {
    let catalog = SkillCatalog::from_skills([
        SkillDescriptor::new(
            "conversation-summary",
            layout
                .skills_dir
                .join("conversation-summary/SKILL.md")
                .display()
                .to_string(),
        )
        .with_description("为 Chat 项目生成会话摘要与待办。"),
        SkillDescriptor::new(
            "knowledge-curator",
            layout
                .skills_dir
                .join("knowledge-curator/SKILL.md")
                .display()
                .to_string(),
        )
        .with_description("辅助整理知识库文档、分块与标签。"),
    ]);
    let sandbox_cache = SkillSandboxCache::from_entries([
        SkillSandboxEntry::new("preset-writer").with_description("Sandbox 写作技能模板。"),
        SkillSandboxEntry::new("preset-research").with_description("Sandbox 检索技能模板。"),
    ]);

    ManagementSkillState::new(catalog).with_sandbox_cache(sandbox_cache, true)
}

fn default_subagent_state(store: SqliteJsonStore) -> AstrbotResult<ManagementSubagentState> {
    ManagementSubagentState::sqlite(
        store,
        ManagementSubagentConfig {
            agents: vec![
                SubagentConfig::new("researcher")
                    .with_public_description("Search and summarize operational context.")
                    .with_tools(["astr_kb_search"]),
                SubagentConfig::new("writer")
                    .with_public_description("Draft concise dashboard-facing responses.")
                    .with_tools(["conversation-summary"]),
            ],
            ..ManagementSubagentConfig::default()
        },
    )
}

fn default_backup_state(
    current_version: impl Into<String>,
    layout: &astrbot_runtime::RuntimePathLayout,
    sqlite_path: impl Into<std::path::PathBuf>,
) -> ManagementBackupState {
    let current_version = current_version.into();
    let sqlite_path = sqlite_path.into();
    let backup_root = layout.backups_dir.clone();
    let chunk_root = backup_root.join("chunks");
    let repository = SqliteBackupRepository::new(sqlite_path.clone(), backup_root.clone())
        .with_directory("config", layout.config_dir.clone())
        .with_directory("plugins", layout.plugin_dir.clone())
        .with_directory("plugin_data", layout.plugin_data_dir.clone())
        .with_directory("knowledge_base", layout.knowledge_base_dir.clone())
        .with_directory("attachments", layout.attachment_dir.clone());
    let importer = SqliteBackupImporter::new(current_version, sqlite_path)
        .with_directory("config", layout.config_dir.clone())
        .with_directory("plugins", layout.plugin_dir.clone())
        .with_directory("plugin_data", layout.plugin_data_dir.clone())
        .with_directory("knowledge_base", layout.knowledge_base_dir.clone())
        .with_directory("attachments", layout.attachment_dir.clone());

    ManagementBackupState::with_roots(
        Arc::new(BackupJobService::new(
            Arc::new(repository),
            Arc::new(FilesystemBackupExporter::new(backup_root.clone())),
            Arc::new(importer),
        )),
        backup_root,
        chunk_root,
    )
}

async fn default_observability_state(
    layout: &astrbot_runtime::RuntimePathLayout,
) -> Result<ManagementObservabilityState, Box<dyn Error>> {
    let logs = Arc::new(InMemoryLogBuffer::new(256));
    logs.push(LogEntry::new(
        LogLevel::Info,
        LogSource::Dashboard,
        "dashboard management server prepared",
    ))
    .await;
    logs.push(LogEntry::new(
        LogLevel::Info,
        LogSource::Runtime,
        "runtime managers are exposed through /api/management/status",
    ))
    .await;
    logs.push(
        LogEntry::new(
            LogLevel::Warn,
            LogSource::Dashboard,
            "update, plugin market and skills still expose safe plan or in-memory closures",
        )
        .with_target("capabilities"),
    )
    .await;
    let traces = vec![TraceEvent {
        span_id: "dashboard-boot".to_string(),
        span_name: "management.dashboard".to_string(),
        action: "prepared".to_string(),
        message_origin: Some("astrbot-cli".to_string()),
        sender_name: Some("dashboard".to_string()),
        message_outline: Some("webchat management server started".to_string()),
        fields: vec![
            ("version".to_string(), DASHBOARD_RUNTIME_VERSION.to_string()),
            ("closure".to_string(), "in_memory".to_string()),
        ],
        occurred_at: SystemTime::now(),
        elapsed: Some(Duration::from_millis(0)),
    }];

    let observability = ManagementObservabilityState::new(logs, traces)
        .with_log_file(layout.data_dir.join("observability/logs.jsonl"))
        .await?
        .with_trace_settings_file(layout.data_dir.join("observability/trace-settings.json"))?
        .with_metric_file(layout.data_dir.join("observability/metrics.jsonl"));
    if !observability.metrics().unwrap_or_default().is_empty() {
        return Ok(observability);
    }

    Ok(observability.with_metrics(vec![
        MetricEvent::platform_message("2026-05-17T08:00:00Z", "webchat", "webchat", 3),
        MetricEvent::llm_response(
            "2026-05-17T08:00:01Z",
            "dashboard-mock",
            UsageRecord::new(32, 8, 16),
        )
        .with_provider_model("mock-chat"),
        MetricEvent::tts_playback(
            "2026-05-17T08:00:02Z",
            "dashboard-tts",
            MetricTtsStats::new(240, 45),
        ),
    ]))
}

async fn default_persona_state(store: SqliteJsonStore) -> ManagementPersonaState {
    let repository = Arc::new(SqlitePersonaRepository::new(store));
    let manager = Arc::new(PersonaManager::with_repository(
        repository,
        PersonaProfile::new("default", "You are a helpful and friendly assistant."),
    ));
    let _ = manager
        .upsert_folder(
            PersonaFolder::new("builtin", "Builtin").with_description("Dashboard presets"),
        )
        .await;
    let _ = manager
        .upsert_persona(
            PersonaProfile::new("support", "Be concise, careful and operational.")
                .with_folder_id("builtin")
                .with_tools(Some(vec!["astr_kb_search".to_string()])),
        )
        .await;
    let _ = manager
        .upsert_persona(
            PersonaProfile::new("creative", "Be vivid, structured and practical.")
                .with_folder_id("builtin")
                .with_skills(Some(vec!["preset-writer".to_string()])),
        )
        .await;

    ManagementPersonaState::new(manager)
}

async fn default_cron_state(
    runtime: &AstrbotRuntime,
    webchat: &Arc<WebChatPlatform>,
    store: SqliteJsonStore,
) -> (ManagementCronState, Arc<CronScheduler>) {
    let event_sink = Arc::new(RuntimeCronEventSink::new(runtime.event_sender()));
    let scheduler = Arc::new(CronScheduler::new(
        Arc::new(SqliteCronJobRepository::new(store)),
        Arc::new(DueCronScheduleDriver::new()),
        Arc::new(ProactiveAgentWakeService::new(event_sink, webchat.sink())),
    ));
    let _ = scheduler
        .add_job(CronJob::active_agent(
            "daily-summary",
            "Daily summary",
            CronJobSchedule::cron("0 8 * * *").with_timezone("Asia/Shanghai"),
            ActiveAgentCronPayload::new("webchat:demo", "生成昨日会话摘要。")
                .with_sender_id("scheduler")
                .with_origin("dashboard"),
        ))
        .await;

    (ManagementCronState::new(scheduler.clone()), scheduler)
}

struct RuntimeCronEventSink {
    event_sender: mpsc::Sender<MessageEvent>,
}

impl RuntimeCronEventSink {
    fn new(event_sender: mpsc::Sender<MessageEvent>) -> Self {
        Self { event_sender }
    }
}

#[async_trait]
impl astrbot_cron::CronEventSink for RuntimeCronEventSink {
    async fn submit(&self, event: MessageEvent) -> AstrbotResult<()> {
        self.event_sender
            .send(event)
            .await
            .map_err(|_| AstrbotError::EventChannelClosed)
    }
}

fn spawn_cron_tick_loop(scheduler: Arc<CronScheduler>) -> JoinHandle<()> {
    tokio::spawn(async move {
        if scheduler.start().await.is_err() {
            return;
        }
        let mut interval = time::interval(Duration::from_secs(30));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let _ = scheduler.tick_due(SystemTime::now()).await;
        }
    })
}

pub(crate) struct WebChatServerHandle {
    address: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<io::Result<()>>,
    cron_task: Option<JoinHandle<()>>,
}

impl WebChatServerHandle {
    pub(crate) fn address(&self) -> SocketAddr {
        self.address
    }

    pub(crate) async fn stop(mut self) -> Result<(), Box<dyn Error>> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(cron_task) = self.cron_task.take() {
            cron_task.abort();
        }

        let result = self
            .task
            .await
            .map_err(|err| io::Error::other(format!("webchat server task join failed: {err}")))?;
        result?;
        Ok(())
    }
}
