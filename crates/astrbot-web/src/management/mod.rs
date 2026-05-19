mod api_key;
mod auth;
mod backup;
mod chat_projects;
mod commands;
mod config;
mod conversations;
mod cron;
mod dashboard;
mod files;
mod knowledge_base;
mod mcp;
mod observability;
mod persona;
mod platforms;
mod plugin_market;
mod plugins;
mod providers;
mod session_rules;
mod skills;
mod status;
mod subagents;
mod t2i_templates;
mod tools;
mod update;

use std::path::PathBuf;
use std::sync::Arc;

use astrbot_conversation::{ConversationService, SqliteConversationDirectory};
use astrbot_core::{MessageChain, MessageSession, MessageSink, MessageStream};
use astrbot_cron::{
    CronScheduler, DueCronScheduleDriver, ProactiveAgentWakeService, RecordingCronEventSink,
    SqliteCronJobRepository,
};
use astrbot_persona::{PersonaManager, PersonaProfile, SqlitePersonaRepository};
use astrbot_platform::PlatformManager;
use astrbot_plugin::PluginRegistry;
use astrbot_provider::ProviderManager;
use astrbot_runtime::RuntimeConfigService;
use astrbot_storage::{
    ApiKeyRepository, AttachmentRepository, ChatProjectRepository, FileTokenRepository,
    PlatformStatsRepository, SessionGroupRepository, SessionRuleRepository, SqliteJsonStore,
    SqliteStorage,
};
use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::State,
    middleware,
    routing::{get, post},
};

pub use api_key::{
    ApiKeyAuthDecision, ApiKeyIssuer, ApiKeyRejectionReason, IssuedApiKey,
    ManagementApiKeyCatalogResponse, ManagementApiKeyDeleteRequest, ManagementApiKeyDeleteResponse,
    ManagementApiKeyDescriptor, ManagementApiKeyIssueRequest, ManagementApiKeyIssueResponse,
    ManagementApiKeyRevokeRequest, ManagementApiKeyRevokeResponse, ManagementApiKeyState,
    OpenApiScope, OpenApiScopeSet, PresentedApiKey, authorize_api_key, extract_presented_api_key,
    hash_api_key,
};
pub use auth::{
    AuthRejectionReason, DashboardAccountEditRequest, DashboardAuthDecision, DashboardAuthPolicy,
    DashboardAuthResponse, DashboardLoginData, DashboardLoginRequest, ManagementActor,
    ManagementAuditEntry, ManagementAuditFileStore, ManagementAuthState, extract_bearer_token,
};
pub use backup::{
    ManagementBackupAbortRequest, ManagementBackupAbortResponse, ManagementBackupChunkRequest,
    ManagementBackupChunkResponse, ManagementBackupCompleteRequest,
    ManagementBackupCompleteResponse, ManagementBackupExportRequest, ManagementBackupImportRequest,
    ManagementBackupJobResponse, ManagementBackupPrecheckRequest, ManagementBackupPrecheckResponse,
    ManagementBackupProgressCatalogResponse, ManagementBackupProgressResponse,
    ManagementBackupState, ManagementBackupUploadStartRequest, ManagementBackupUploadStartResponse,
};
pub use chat_projects::{
    ManagementChatProjectActorRequest, ManagementChatProjectCatalogResponse,
    ManagementChatProjectCreateRequest, ManagementChatProjectDescriptor,
    ManagementChatProjectGetRequest, ManagementChatProjectMembershipRequest,
    ManagementChatProjectMutationResponse, ManagementChatProjectResponse,
    ManagementChatProjectSessionResponse, ManagementChatProjectSessionUpsertRequest,
    ManagementChatProjectSessionsResponse, ManagementChatProjectState,
    ManagementChatProjectUpdateRequest, ManagementPlatformSessionDescriptor,
};
pub use commands::{
    ManagementCommandCatalogResponse, ManagementCommandDescriptor,
    ManagementCommandMutationResponse, ManagementCommandUpdateRequest,
};
pub use config::{
    ManagementAbconfCatalogResponse, ManagementAbconfCreateRequest, ManagementAbconfCreateResponse,
    ManagementAbconfDeleteResponse, ManagementAbconfIdRequest, ManagementAbconfResponse,
    ManagementAbconfUpdateRequest, ManagementConfigApplyExecution,
    ManagementConfigApplyExecutionRequest, ManagementConfigApplyExecutor,
    ManagementConfigApplyFuture, ManagementConfigApplyState, ManagementConfigCurrentResponse,
    ManagementConfigMutationRequest, ManagementConfigMutationResponse,
    ManagementConfigRouteCatalogResponse, ManagementConfigRouteDeleteRequest,
    ManagementConfigRouteMutationResponse, ManagementConfigRouteReplaceRequest,
    ManagementConfigRouteResolveRequest, ManagementConfigRouteResolveResponse,
    ManagementConfigRouteState, ManagementConfigRouteUpsertRequest, ManagementConfigSchemaResponse,
    ManagementRuntimeConfigApplyController,
};
pub use conversations::{
    ManagementConversationBatchDeleteRequest, ManagementConversationBatchDeleteResponse,
    ManagementConversationCatalogResponse, ManagementConversationCurrentRequest,
    ManagementConversationCurrentResponse, ManagementConversationDeleteRequest,
    ManagementConversationDeleteResponse, ManagementConversationDescriptor,
    ManagementConversationGetRequest, ManagementConversationListRequest,
    ManagementConversationMutationResponse, ManagementConversationRenameRequest,
    ManagementConversationResponse, ManagementConversationState,
    ManagementConversationUpsertRequest,
};
pub use cron::{
    ManagementCronCatalogResponse, ManagementCronDeleteResponse, ManagementCronJobRequest,
    ManagementCronListRequest, ManagementCronMutationResponse, ManagementCronState,
    ManagementCronTickRequest, ManagementCronTickResponse, ManagementCronUpsertRequest,
};
pub use dashboard::{
    DashboardCapabilitiesResponse, DashboardClosureLevel, DashboardServiceCapability,
};
pub use files::{
    FileUploadError, ManagementFileDownloadState, ManagementFileUploadResponse,
    ScopedDownloadError, ScopedDownloadFile,
};
pub use knowledge_base::{
    ManagementKnowledgeBaseCatalogResponse, ManagementKnowledgeBaseCreateRequest,
    ManagementKnowledgeBaseIdRequest, ManagementKnowledgeBaseResponse,
    ManagementKnowledgeBaseState, ManagementKnowledgeBaseUpdateRequest,
    ManagementKnowledgeChunkCatalogResponse, ManagementKnowledgeChunkDeleteRequest,
    ManagementKnowledgeDocumentCatalogResponse, ManagementKnowledgeDocumentIdRequest,
    ManagementKnowledgeIngestRequest, ManagementKnowledgeIngestResponse,
    ManagementKnowledgeMutationResponse, ManagementKnowledgePreflightResponse,
    ManagementKnowledgeProviderPreflightRequest, ManagementKnowledgeUploadCompleteRequest,
    ManagementKnowledgeUploadFailRequest, ManagementKnowledgeUploadPlanRequest,
    ManagementKnowledgeUploadProgressRequest, ManagementKnowledgeUploadTaskResponse,
};
pub use mcp::{
    ManagementMcpCatalogResponse, ManagementMcpCheckRequest, ManagementMcpCheckResponse,
    ManagementMcpDeleteRequest, ManagementMcpMutationResponse, ManagementMcpServerConfigView,
    ManagementMcpServerDescriptor, ManagementMcpState, ManagementMcpSyncRequest,
    ManagementMcpSyncResponse, ManagementMcpUpsertRequest,
};
pub use observability::{
    ManagementLogQuery, ManagementLogResponse, ManagementMetricFileStore,
    ManagementObservabilityState, ManagementPlatformStatsSummary, ManagementProviderUsageSummary,
    ManagementStatsResponse, ManagementTraceResponse,
};
pub use persona::{
    ManagementPersonaCatalogResponse, ManagementPersonaFolderUpsertRequest,
    ManagementPersonaListRequest, ManagementPersonaMutationResponse,
    ManagementPersonaResolveRequest, ManagementPersonaResolveResponse, ManagementPersonaState,
    ManagementPersonaUpsertRequest,
};
pub use platforms::{
    DefaultManagementPlatformHealthCheck, ManagementPlatformCatalogResponse,
    ManagementPlatformCheckRequest, ManagementPlatformCheckResponse,
    ManagementPlatformDeleteRequest, ManagementPlatformDescriptor, ManagementPlatformHealthCheck,
    ManagementPlatformHealthFuture, ManagementPlatformHealthResult,
    ManagementPlatformMutationResponse, ManagementPlatformTemplate,
    ManagementPlatformUpsertRequest, PlatformManagementResponse,
};
pub use plugin_market::{
    PluginMarketCatalogResponse, PluginMarketExecuteResponse, PluginMarketInstalledPlugin,
    PluginMarketManagementState, PluginMarketOperationRecord, PluginMarketOperationStatus,
    PluginMarketPlanRequest, PluginMarketPlanResponse, PluginMarketPluginDescriptor,
    PluginMarketUpdateAllExecuteResponse, PluginMarketUpdateAllPlanResponse,
    PluginMarketUpdateAllRequest,
};
pub use plugins::{
    ManagementPluginAction, ManagementPluginConfigFileDeleteResponse,
    ManagementPluginConfigFileDescriptor, ManagementPluginConfigFileListRequest,
    ManagementPluginConfigFileListResponse, ManagementPluginConfigFileReadResponse,
    ManagementPluginConfigFileRequest, ManagementPluginConfigFileWriteRequest,
    ManagementPluginConfigFileWriteResponse, ManagementPluginConfigRequest,
    ManagementPluginDescriptor, ManagementPluginLifecycleActionRequest,
    ManagementPluginLifecycleCatalogResponse, ManagementPluginLifecycleEventDescriptor,
    ManagementPluginLifecycleMutationResponse, ManagementPluginLifecycleState,
    ManagementPluginOperationRecord, ManagementPluginSeed, ManagementPluginSourceDescriptor,
    ManagementPluginSourcePlanRequest, ManagementPluginSourcePlanResponse,
    ManagementPluginUploadPlanRequest, ManagementPluginUploadPlanResponse,
    PluginHandlerManagementResponse, PluginManagementResponse,
};
pub use providers::{
    DefaultManagementProviderHealthCheck, ManagementChatProviderDescriptor,
    ManagementProviderCatalogResponse, ManagementProviderCheckRequest,
    ManagementProviderCheckResponse, ManagementProviderDeleteRequest,
    ManagementProviderHealthCheck, ManagementProviderHealthFuture, ManagementProviderHealthResult,
    ManagementProviderModelsFuture, ManagementProviderModelsRequest,
    ManagementProviderModelsResponse, ManagementProviderModelsResult,
    ManagementProviderMutationResponse, ManagementProviderTemplate,
    ManagementProviderUpsertRequest, ProviderManagementResponse,
};
pub use session_rules::ManagementSessionRuleState;
pub use skills::{
    ManagementSkillActivationRequest, ManagementSkillActivationResponse,
    ManagementSkillCatalogResponse, ManagementSkillDeletePlanRequest,
    ManagementSkillDeletePlanResponse, ManagementSkillDeleteResponse, ManagementSkillDescriptor,
    ManagementSkillInstallPlanRequest, ManagementSkillInstallPlanResponse,
    ManagementSkillInstallResponse, ManagementSkillState,
};
pub use status::ManagementStatusResponse;
pub use subagents::{
    ManagementSubagentApplyRequest, ManagementSubagentApplyResponse,
    ManagementSubagentCatalogResponse, ManagementSubagentConfig, ManagementSubagentDescriptor,
    ManagementSubagentExecuteRequest, ManagementSubagentExecuteResponse,
    ManagementSubagentExecutionBridge, ManagementSubagentExecutionRecord,
    ManagementSubagentExecutionResult, ManagementSubagentHandoffDescriptor,
    ManagementSubagentState,
};
pub use t2i_templates::ManagementT2iTemplateState;
pub use tools::{
    ManagementToolCatalogResponse, ManagementToolDescriptor, ManagementToolState,
    ManagementToolToggleRequest, ManagementToolToggleResponse,
};
pub use update::{
    DashboardUpdatePlanRequest, LocalMaintenanceExecutor, MaintenanceChangelogResponse,
    MaintenanceCheckResponse, MaintenanceExecutionFuture, MaintenanceMigrationCheckResponse,
    MaintenanceMigrationExecutor, MaintenanceOperationResponse, MaintenanceOperationRunRequest,
    MaintenanceOperationsResponse, MaintenancePackageExecutor, MaintenancePackagePlanResponse,
    MaintenanceReleaseExecutor, MaintenanceRestartExecutor, MaintenanceRestartRequest,
    ManagementMaintenanceState, ProjectUpdatePlanRequest,
};

#[derive(Clone)]
pub struct ManagementApiState {
    providers: ProviderManagementResponse,
    provider_manager: Option<ProviderManager>,
    provider_health_check: Arc<dyn ManagementProviderHealthCheck>,
    platforms: PlatformManagementResponse,
    platform_health_check: Arc<dyn ManagementPlatformHealthCheck>,
    plugins: PluginManagementResponse,
    plugin_lifecycle: Option<ManagementPluginLifecycleState>,
    config_service: Option<RuntimeConfigService>,
    config_routes: Option<ManagementConfigRouteState>,
    config_apply: Option<ManagementConfigApplyState>,
    plugin_market: Option<PluginMarketManagementState>,
    file_downloads: Option<ManagementFileDownloadState>,
    backup: Option<ManagementBackupState>,
    conversations: Option<ManagementConversationState>,
    chat_projects: Option<ManagementChatProjectState>,
    session_rules: Option<ManagementSessionRuleState>,
    skills: Option<ManagementSkillState>,
    tools: Option<ManagementToolState>,
    mcp: Option<ManagementMcpState>,
    maintenance: Option<ManagementMaintenanceState>,
    knowledge_base: Option<ManagementKnowledgeBaseState>,
    observability: Option<ManagementObservabilityState>,
    personas: Option<ManagementPersonaState>,
    cron: Option<ManagementCronState>,
    subagents: Option<ManagementSubagentState>,
    t2i_templates: Option<ManagementT2iTemplateState>,
    api_keys: Option<ManagementApiKeyState>,
}

impl std::fmt::Debug for ManagementApiState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementApiState")
            .finish_non_exhaustive()
    }
}

struct NoopManagementMessageSink;

#[async_trait]
impl MessageSink for NoopManagementMessageSink {
    async fn send(
        &self,
        _session: &MessageSession,
        _chain: MessageChain,
    ) -> astrbot_core::Result<()> {
        Ok(())
    }

    async fn send_streaming(
        &self,
        _session: &MessageSession,
        _stream: MessageStream,
    ) -> astrbot_core::Result<()> {
        Ok(())
    }
}

impl ManagementApiState {
    pub fn new(
        providers: ProviderManagementResponse,
        platforms: PlatformManagementResponse,
        plugins: PluginManagementResponse,
    ) -> Self {
        Self {
            providers,
            provider_manager: None,
            provider_health_check: Arc::new(DefaultManagementProviderHealthCheck),
            platforms,
            platform_health_check: Arc::new(DefaultManagementPlatformHealthCheck),
            plugins,
            plugin_lifecycle: None,
            config_service: None,
            config_routes: None,
            config_apply: None,
            plugin_market: None,
            file_downloads: None,
            backup: None,
            conversations: None,
            chat_projects: None,
            session_rules: None,
            skills: None,
            tools: None,
            mcp: None,
            maintenance: None,
            knowledge_base: None,
            observability: None,
            personas: None,
            cron: None,
            subagents: None,
            t2i_templates: None,
            api_keys: None,
        }
    }

    pub fn with_config_service(mut self, config_service: RuntimeConfigService) -> Self {
        if self.t2i_templates.is_none()
            && let Ok(t2i_templates) =
                ManagementT2iTemplateState::from_config_service(config_service.clone())
        {
            self.t2i_templates = Some(t2i_templates);
        }
        self.config_service = Some(config_service);
        self
    }

    pub fn with_config_routes(mut self, config_routes: ManagementConfigRouteState) -> Self {
        self.config_routes = Some(config_routes);
        self
    }

    pub fn with_config_apply(mut self, config_apply: ManagementConfigApplyState) -> Self {
        self.config_apply = Some(config_apply);
        self
    }

    pub fn with_provider_health_check(
        mut self,
        provider_health_check: Arc<dyn ManagementProviderHealthCheck>,
    ) -> Self {
        self.provider_health_check = provider_health_check;
        self
    }

    pub fn with_platform_health_check(
        mut self,
        platform_health_check: Arc<dyn ManagementPlatformHealthCheck>,
    ) -> Self {
        self.platform_health_check = platform_health_check;
        self
    }

    pub fn with_plugin_market(mut self, plugin_market: PluginMarketManagementState) -> Self {
        self.plugin_market = Some(plugin_market);
        self
    }

    pub fn with_plugin_lifecycle(
        mut self,
        plugin_lifecycle: ManagementPluginLifecycleState,
    ) -> Self {
        self.plugin_lifecycle = Some(plugin_lifecycle);
        self
    }

    pub fn with_file_downloads(mut self, file_downloads: ManagementFileDownloadState) -> Self {
        self.file_downloads = Some(file_downloads);
        self
    }

    pub fn with_file_storage_roots(
        mut self,
        attachment_dir: impl Into<PathBuf>,
        temp_dir: impl Into<PathBuf>,
    ) -> Self {
        if let Some(file_downloads) = self.file_downloads.take() {
            self.file_downloads = Some(file_downloads.with_file_roots(attachment_dir, temp_dir));
        }
        self
    }

    pub fn with_file_allowed_root(mut self, root: impl Into<PathBuf>) -> Self {
        if let Some(file_downloads) = self.file_downloads.take() {
            self.file_downloads = Some(file_downloads.with_allowed_root(root));
        }
        self
    }

    pub fn with_backup(mut self, backup: ManagementBackupState) -> Self {
        self.backup = Some(backup);
        self
    }

    pub fn with_conversations(mut self, conversations: ManagementConversationState) -> Self {
        self.conversations = Some(conversations);
        self
    }

    pub fn with_chat_projects(mut self, chat_projects: ManagementChatProjectState) -> Self {
        self.chat_projects = Some(chat_projects);
        self
    }

    pub fn with_session_rules(mut self, session_rules: ManagementSessionRuleState) -> Self {
        self.session_rules = Some(session_rules);
        self
    }

    pub fn with_skills(mut self, skills: ManagementSkillState) -> Self {
        self.skills = Some(skills);
        self
    }

    pub fn with_tools(mut self, tools: ManagementToolState) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_mcp(mut self, mcp: ManagementMcpState) -> Self {
        self.mcp = Some(mcp);
        self
    }

    pub fn with_maintenance(mut self, maintenance: ManagementMaintenanceState) -> Self {
        self.maintenance = Some(maintenance);
        self
    }

    pub fn with_knowledge_base(mut self, knowledge_base: ManagementKnowledgeBaseState) -> Self {
        self.knowledge_base = Some(knowledge_base);
        self
    }

    pub fn with_observability(mut self, observability: ManagementObservabilityState) -> Self {
        self.observability = Some(observability);
        self
    }

    pub fn with_personas(mut self, personas: ManagementPersonaState) -> Self {
        self.personas = Some(personas);
        self
    }

    pub fn with_cron(mut self, cron: ManagementCronState) -> Self {
        self.cron = Some(cron);
        self
    }

    pub fn with_subagents(mut self, subagents: ManagementSubagentState) -> Self {
        self.subagents = Some(subagents);
        self
    }

    pub fn with_t2i_templates(mut self, t2i_templates: ManagementT2iTemplateState) -> Self {
        self.t2i_templates = Some(t2i_templates);
        self
    }

    pub fn with_api_keys(mut self, api_keys: ManagementApiKeyState) -> Self {
        self.api_keys = Some(api_keys);
        self
    }

    pub fn with_sqlite_storage_path(self, path: impl Into<PathBuf>) -> astrbot_core::Result<Self> {
        let path = path.into();
        let data_dir = path.parent().map(PathBuf::from);
        let storage = Arc::new(SqliteStorage::open(path.clone())?);
        let json_store = SqliteJsonStore::open(path)?;
        let mut state = self.with_sqlite_storage(storage, json_store);
        if let Some(data_dir) = data_dir {
            state =
                state.with_file_storage_roots(data_dir.join("attachments"), data_dir.join("temp"));
        }
        Ok(state)
    }

    pub fn with_sqlite_storage(
        mut self,
        storage: Arc<SqliteStorage>,
        json_store: SqliteJsonStore,
    ) -> Self {
        if self.api_keys.is_none() {
            let repository: Arc<dyn ApiKeyRepository> = storage.clone();
            self.api_keys = Some(ManagementApiKeyState::new(repository));
        }
        if self.file_downloads.is_none() {
            let repository: Arc<dyn FileTokenRepository> = storage.clone();
            let attachments: Arc<dyn AttachmentRepository> = storage.clone();
            self.file_downloads = Some(
                ManagementFileDownloadState::new(repository)
                    .with_attachment_repository(attachments),
            );
        }
        if self.conversations.is_none() {
            self.conversations = Some(ManagementConversationState::new(
                ConversationService::with_directory(Arc::new(SqliteConversationDirectory::new(
                    json_store.clone(),
                ))),
            ));
        }
        if self.personas.is_none() {
            let repository = Arc::new(SqlitePersonaRepository::new(json_store.clone()));
            self.personas = Some(ManagementPersonaState::new(Arc::new(
                PersonaManager::with_repository(
                    repository,
                    PersonaProfile::new("default", "You are a helpful and friendly assistant."),
                ),
            )));
        }
        if self.chat_projects.is_none() {
            let repository: Arc<dyn ChatProjectRepository> = storage.clone();
            self.chat_projects = Some(ManagementChatProjectState::new(
                astrbot_conversation::ChatProjectService::new(repository),
            ));
        }
        if self.session_rules.is_none() {
            let rules: Arc<dyn SessionRuleRepository> = storage.clone();
            let groups: Arc<dyn SessionGroupRepository> = storage.clone();
            self.session_rules = Some(ManagementSessionRuleState::new(rules, groups));
        }
        if self.knowledge_base.is_none() {
            self.knowledge_base = Some(ManagementKnowledgeBaseState::sqlite(
                self.provider_manager.clone().unwrap_or_default(),
                json_store.clone(),
            ));
        }
        if self.cron.is_none() {
            self.cron = Some(ManagementCronState::new(Arc::new(CronScheduler::new(
                Arc::new(SqliteCronJobRepository::new(json_store)),
                Arc::new(DueCronScheduleDriver::new()),
                Arc::new(ProactiveAgentWakeService::new(
                    Arc::new(RecordingCronEventSink::new()),
                    Arc::new(NoopManagementMessageSink),
                )),
            ))));
        }
        if let Some(observability) = self.observability.take() {
            let stats: Arc<dyn PlatformStatsRepository> = storage;
            self.observability = Some(observability.with_platform_stats_repository(stats));
        }
        self
    }

    pub fn from_managers(
        provider_manager: &ProviderManager,
        platform_manager: &PlatformManager,
        plugin_registry: &PluginRegistry,
    ) -> Self {
        let mut state = Self::new(
            ProviderManagementResponse::from_manager(provider_manager),
            PlatformManagementResponse::from_manager(platform_manager),
            PluginManagementResponse::from_registry(plugin_registry),
        );
        state.provider_manager = Some(provider_manager.clone());
        state
    }

    pub fn providers(&self) -> &ProviderManagementResponse {
        &self.providers
    }

    pub fn provider_health_check(&self) -> &Arc<dyn ManagementProviderHealthCheck> {
        &self.provider_health_check
    }

    pub fn platforms(&self) -> &PlatformManagementResponse {
        &self.platforms
    }

    pub fn platform_health_check(&self) -> &Arc<dyn ManagementPlatformHealthCheck> {
        &self.platform_health_check
    }

    pub fn plugins(&self) -> &PluginManagementResponse {
        &self.plugins
    }

    pub fn plugin_lifecycle(&self) -> Option<&ManagementPluginLifecycleState> {
        self.plugin_lifecycle.as_ref()
    }

    pub fn config_service(&self) -> Option<&RuntimeConfigService> {
        self.config_service.as_ref()
    }

    pub fn config_routes(&self) -> Option<&ManagementConfigRouteState> {
        self.config_routes.as_ref()
    }

    pub fn config_apply(&self) -> Option<&ManagementConfigApplyState> {
        self.config_apply.as_ref()
    }

    pub fn plugin_market(&self) -> Option<&PluginMarketManagementState> {
        self.plugin_market.as_ref()
    }

    pub fn file_downloads(&self) -> Option<&ManagementFileDownloadState> {
        self.file_downloads.as_ref()
    }

    pub fn backup(&self) -> Option<&ManagementBackupState> {
        self.backup.as_ref()
    }

    pub fn conversations(&self) -> Option<&ManagementConversationState> {
        self.conversations.as_ref()
    }

    pub fn chat_projects(&self) -> Option<&ManagementChatProjectState> {
        self.chat_projects.as_ref()
    }

    pub fn session_rules(&self) -> Option<&ManagementSessionRuleState> {
        self.session_rules.as_ref()
    }

    pub fn skills(&self) -> Option<&ManagementSkillState> {
        self.skills.as_ref()
    }

    pub fn tools(&self) -> Option<&ManagementToolState> {
        self.tools.as_ref()
    }

    pub fn mcp(&self) -> Option<&ManagementMcpState> {
        self.mcp.as_ref()
    }

    pub fn maintenance(&self) -> Option<&ManagementMaintenanceState> {
        self.maintenance.as_ref()
    }

    pub fn knowledge_base(&self) -> Option<&ManagementKnowledgeBaseState> {
        self.knowledge_base.as_ref()
    }

    pub fn observability(&self) -> Option<&ManagementObservabilityState> {
        self.observability.as_ref()
    }

    pub fn personas(&self) -> Option<&ManagementPersonaState> {
        self.personas.as_ref()
    }

    pub fn cron(&self) -> Option<&ManagementCronState> {
        self.cron.as_ref()
    }

    pub fn subagents(&self) -> Option<&ManagementSubagentState> {
        self.subagents.as_ref()
    }

    pub fn t2i_templates(&self) -> Option<&ManagementT2iTemplateState> {
        self.t2i_templates.as_ref()
    }

    pub fn api_keys(&self) -> Option<&ManagementApiKeyState> {
        self.api_keys.as_ref()
    }

    pub fn status(&self) -> ManagementStatusResponse {
        ManagementStatusResponse::new(
            self.providers.clone(),
            self.platforms.clone(),
            self.plugins.clone(),
        )
    }
}

pub fn management_router(state: ManagementApiState) -> Router {
    management_routes().with_state(state)
}

pub fn management_router_with_auth(state: ManagementApiState, auth: ManagementAuthState) -> Router {
    let protected_auth = auth.clone().with_api_keys(state.api_keys().cloned());
    let protected = management_routes()
        .route_layer(middleware::from_fn_with_state(
            protected_auth,
            auth::require_management_auth,
        ))
        .with_state(state);
    auth_routes(auth).merge(protected)
}

fn auth_routes(auth: ManagementAuthState) -> Router {
    Router::new()
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/account/edit", post(auth::edit_account))
        .with_state(auth)
}

fn management_routes() -> Router<ManagementApiState> {
    Router::new()
        .route(
            "/api/management/dashboard/capabilities",
            get(dashboard::capabilities),
        )
        .route("/api/management/status", get(status))
        .route("/api/management/providers", get(providers))
        .route("/api/management/providers/catalog", get(providers::catalog))
        .route("/api/management/providers/upsert", post(providers::upsert))
        .route("/api/management/providers/delete", post(providers::delete))
        .route("/api/management/providers/check", post(providers::check))
        .route("/api/management/providers/models", post(providers::models))
        .route(
            "/api/config/provider/template",
            get(providers::legacy_template),
        )
        .route("/api/config/provider/list", get(providers::legacy_list))
        .route("/api/config/provider/new", post(providers::legacy_create))
        .route(
            "/api/config/provider/update",
            post(providers::legacy_update),
        )
        .route(
            "/api/config/provider/delete",
            post(providers::legacy_delete),
        )
        .route(
            "/api/config/provider/check_one",
            get(providers::legacy_check_one),
        )
        .route(
            "/api/config/provider/model_list",
            get(providers::legacy_model_list),
        )
        .route(
            "/api/config/provider/get_embedding_dim",
            post(providers::legacy_embedding_dim),
        )
        .route(
            "/api/config/provider_sources/models",
            get(providers::legacy_source_models),
        )
        .route(
            "/api/config/provider_sources/update",
            post(providers::legacy_source_update),
        )
        .route(
            "/api/config/provider_sources/delete",
            post(providers::legacy_source_delete),
        )
        .route("/api/management/platforms", get(platforms))
        .route("/api/management/platforms/catalog", get(platforms::catalog))
        .route("/api/management/platforms/upsert", post(platforms::upsert))
        .route("/api/management/platforms/delete", post(platforms::delete))
        .route("/api/management/platforms/check", post(platforms::check))
        .route("/api/config/platform/new", post(platforms::legacy_create))
        .route(
            "/api/config/platform/update",
            post(platforms::legacy_update),
        )
        .route(
            "/api/config/platform/delete",
            post(platforms::legacy_delete),
        )
        .route("/api/platform/stats", get(platforms::legacy_stats))
        .route(
            "/api/platform/webhook/{webhook_uuid}",
            get(platforms::legacy_webhook).post(platforms::legacy_webhook),
        )
        .route("/api/management/plugins", get(plugins))
        .route(
            "/api/management/plugins/lifecycle",
            get(plugins::lifecycle_catalog),
        )
        .route(
            "/api/management/plugins/lifecycle/action",
            post(plugins::lifecycle_action),
        )
        .route(
            "/api/management/plugins/upload-plan",
            post(plugins::upload_plan),
        )
        .route(
            "/api/management/plugins/source-plan",
            post(plugins::source_plan),
        )
        .route("/api/management/plugins/config", post(plugins::save_config))
        .route(
            "/api/management/plugins/config-file/list",
            post(plugins::list_config_files),
        )
        .route(
            "/api/management/plugins/config-file/read",
            post(plugins::read_config_file),
        )
        .route(
            "/api/management/plugins/config-file/write",
            post(plugins::write_config_file),
        )
        .route(
            "/api/management/plugins/config-file/delete",
            post(plugins::delete_config_file),
        )
        .route("/api/plugin/get", get(plugins::legacy_get))
        .route(
            "/api/plugin/check-compat",
            post(plugin_market::legacy_check_compat),
        )
        .route("/api/plugin/install", post(plugin_market::legacy_install))
        .route(
            "/api/plugin/install-upload",
            post(plugin_market::legacy_install),
        )
        .route("/api/plugin/update", post(plugin_market::legacy_update))
        .route(
            "/api/plugin/update-all",
            post(plugin_market::legacy_update_all),
        )
        .route(
            "/api/plugin/uninstall",
            post(plugin_market::legacy_uninstall),
        )
        .route(
            "/api/plugin/market_list",
            get(plugin_market::legacy_market_list),
        )
        .route("/api/plugin/on", post(plugins::legacy_on))
        .route("/api/plugin/off", post(plugins::legacy_off))
        .route("/api/plugin/reload", post(plugins::legacy_reload))
        .route(
            "/api/plugin/reload-failed",
            post(plugins::legacy_reload_failed),
        )
        .route(
            "/api/plugin/uninstall-failed",
            post(plugins::legacy_uninstall_failed),
        )
        .route("/api/plugin/readme", get(plugins::legacy_readme))
        .route("/api/plugin/changelog", get(plugins::legacy_changelog))
        .route("/api/plugin/source/get", get(plugins::legacy_source_get))
        .route("/api/plugin/source/save", post(plugins::legacy_source_save))
        .route(
            "/api/plugin/source/get-failed-plugins",
            get(plugins::legacy_failed_plugins),
        )
        .route("/api/management/logs", get(observability::logs))
        .route(
            "/api/management/logs/stream",
            get(observability::logs_stream),
        )
        .route("/api/live-log", get(observability::legacy_live_log))
        .route("/api/log-history", get(observability::legacy_log_history))
        .route("/api/management/logs/push", post(observability::push_log))
        .route("/api/management/trace", get(observability::trace))
        .route(
            "/api/management/trace/settings",
            get(observability::trace_settings),
        )
        .route(
            "/api/management/trace/settings",
            post(observability::update_trace_settings),
        )
        .route(
            "/api/trace/settings",
            get(observability::legacy_trace_settings),
        )
        .route(
            "/api/trace/settings",
            post(observability::legacy_update_trace_settings),
        )
        .route("/api/management/stats", get(observability::stats))
        .route(
            "/api/management/stats/push",
            post(observability::push_metric),
        )
        .route("/api/management/personas", post(persona::catalog))
        .route("/api/management/personas/upsert", post(persona::upsert))
        .route("/api/management/personas/delete", post(persona::delete))
        .route("/api/management/personas/move", post(persona::move_persona))
        .route(
            "/api/management/personas/clone",
            post(persona::clone_persona),
        )
        .route("/api/management/personas/reorder", post(persona::reorder))
        .route(
            "/api/management/personas/folders/upsert",
            post(persona::upsert_folder),
        )
        .route(
            "/api/management/personas/folders/delete",
            post(persona::delete_folder),
        )
        .route(
            "/api/management/personas/folders/move",
            post(persona::move_folder),
        )
        .route("/api/management/personas/resolve", post(persona::resolve))
        .route("/api/persona/list", get(persona::legacy_list))
        .route("/api/persona/detail", post(persona::legacy_detail))
        .route("/api/persona/create", post(persona::legacy_create))
        .route("/api/persona/update", post(persona::legacy_update))
        .route("/api/persona/delete", post(persona::legacy_delete))
        .route("/api/persona/clone", post(persona::legacy_clone))
        .route("/api/persona/move", post(persona::legacy_move))
        .route("/api/persona/reorder", post(persona::legacy_reorder))
        .route("/api/persona/folder/list", get(persona::legacy_folder_list))
        .route("/api/persona/folder/tree", get(persona::legacy_folder_tree))
        .route(
            "/api/persona/folder/detail",
            post(persona::legacy_folder_detail),
        )
        .route(
            "/api/persona/folder/create",
            post(persona::legacy_folder_create),
        )
        .route(
            "/api/persona/folder/update",
            post(persona::legacy_folder_update),
        )
        .route(
            "/api/persona/folder/delete",
            post(persona::legacy_folder_delete),
        )
        .route("/api/management/cron/jobs", post(cron::list))
        .route("/api/management/cron/jobs/upsert", post(cron::upsert))
        .route("/api/management/cron/jobs/run", post(cron::run))
        .route("/api/management/cron/tick", post(cron::tick))
        .route("/api/management/cron/jobs/delete", post(cron::delete))
        .route("/api/management/cron/start", post(cron::start))
        .route("/api/management/cron/shutdown", post(cron::shutdown))
        .route(
            "/api/cron/jobs",
            get(cron::legacy_list).post(cron::legacy_create),
        )
        .route("/api/cron/jobs/{job_id}/run", post(cron::legacy_run))
        .route(
            "/api/cron/jobs/{job_id}",
            axum::routing::patch(cron::legacy_update).delete(cron::legacy_delete),
        )
        .route("/api/management/subagents", get(subagents::catalog))
        .route("/api/management/subagents/apply", post(subagents::apply))
        .route(
            "/api/management/subagents/execute",
            post(subagents::execute),
        )
        .route(
            "/api/subagent/config",
            get(subagents::source_config).post(subagents::source_update_config),
        )
        .route(
            "/api/subagent/available-tools",
            get(subagents::source_available_tools),
        )
        .route("/api/management/api-keys", get(api_key::catalog))
        .route("/api/management/api-keys/issue", post(api_key::issue))
        .route("/api/management/api-keys/revoke", post(api_key::revoke))
        .route("/api/management/api-keys/delete", post(api_key::delete))
        .route("/api/v1/apikeys", get(api_key::legacy_catalog))
        .route("/api/v1/apikeys", post(api_key::legacy_issue))
        .route(
            "/api/v1/apikeys/{key_id}/revoke",
            post(api_key::legacy_revoke_path),
        )
        .route(
            "/api/v1/apikeys/{key_id}",
            axum::routing::delete(api_key::legacy_delete_path),
        )
        .route("/api/apikey/list", get(api_key::legacy_catalog))
        .route("/api/apikey/create", post(api_key::legacy_issue))
        .route("/api/apikey/revoke", post(api_key::legacy_revoke))
        .route("/api/apikey/delete", post(api_key::legacy_delete))
        .route("/api/management/config/current", get(config::current))
        .route("/api/management/config/schema", get(config::schema))
        .route(
            "/api/management/config/abconfs",
            get(config::abconf_catalog),
        )
        .route(
            "/api/management/config/abconfs/create",
            post(config::abconf_create),
        )
        .route(
            "/api/management/config/abconfs/get",
            post(config::abconf_get),
        )
        .route(
            "/api/management/config/abconfs/update",
            post(config::abconf_update),
        )
        .route(
            "/api/management/config/abconfs/delete",
            post(config::abconf_delete),
        )
        .route("/api/management/config/routes", get(config::route_catalog))
        .route(
            "/api/management/config/routes/upsert",
            post(config::route_upsert),
        )
        .route(
            "/api/management/config/routes/delete",
            post(config::route_delete),
        )
        .route(
            "/api/management/config/routes/replace",
            post(config::route_replace),
        )
        .route(
            "/api/management/config/routes/resolve",
            post(config::route_resolve),
        )
        .route(
            "/api/management/config/preview",
            post(config::preview_update),
        )
        .route("/api/management/config/apply", post(config::apply_update))
        .route("/api/config/abconfs", get(config::legacy_abconf_catalog))
        .route("/api/config/abconf/new", post(config::legacy_abconf_create))
        .route("/api/config/abconf", get(config::legacy_abconf_get))
        .route(
            "/api/config/abconf/update",
            post(config::legacy_abconf_update),
        )
        .route(
            "/api/config/abconf/delete",
            post(config::legacy_abconf_delete),
        )
        .route("/api/config/default", get(config::legacy_default_config))
        .route("/api/config/get", get(config::legacy_current_config))
        .route(
            "/api/config/astrbot/update",
            post(config::legacy_apply_update),
        )
        .route(
            "/api/config/umo_abconf_routes",
            get(config::legacy_route_catalog),
        )
        .route(
            "/api/config/umo_abconf_route/update_all",
            post(config::legacy_route_replace),
        )
        .route(
            "/api/config/umo_abconf_route/update",
            post(config::legacy_route_upsert),
        )
        .route(
            "/api/config/umo_abconf_route/delete",
            post(config::legacy_route_delete),
        )
        .route("/api/t2i/templates", get(t2i_templates::list_templates))
        .route(
            "/api/t2i/templates/active",
            get(t2i_templates::active_template),
        )
        .route(
            "/api/t2i/templates/create",
            post(t2i_templates::create_template),
        )
        .route(
            "/api/t2i/templates/reset_default",
            post(t2i_templates::reset_default_template),
        )
        .route(
            "/api/t2i/templates/set_active",
            post(t2i_templates::set_active_template),
        )
        .route(
            "/api/t2i/templates/{name}",
            get(t2i_templates::get_template)
                .put(t2i_templates::update_template)
                .delete(t2i_templates::delete_template),
        )
        .route("/api/management/plugin-market", get(plugin_market::catalog))
        .route("/api/management/conversations", post(conversations::list))
        .route(
            "/api/management/conversations/get",
            post(conversations::get),
        )
        .route(
            "/api/management/conversations/upsert",
            post(conversations::upsert),
        )
        .route(
            "/api/management/conversations/rename",
            post(conversations::rename),
        )
        .route(
            "/api/management/conversations/current",
            post(conversations::current),
        )
        .route(
            "/api/management/conversations/delete",
            post(conversations::delete),
        )
        .route(
            "/api/management/conversations/batch-delete",
            post(conversations::batch_delete),
        )
        .route("/api/conversation/list", get(conversations::source_list))
        .route(
            "/api/conversation/detail",
            post(conversations::source_detail),
        )
        .route(
            "/api/conversation/update",
            post(conversations::source_update),
        )
        .route(
            "/api/conversation/delete",
            post(conversations::source_delete),
        )
        .route(
            "/api/conversation/update_history",
            post(conversations::source_update_history),
        )
        .route(
            "/api/conversation/export",
            post(conversations::source_export),
        )
        .route("/api/management/chat-projects", post(chat_projects::list))
        .route(
            "/api/management/session-rules",
            get(session_rules::list_rules),
        )
        .route(
            "/api/management/session-rules/update",
            post(session_rules::update_rule),
        )
        .route(
            "/api/management/session-rules/delete",
            post(session_rules::delete_rule),
        )
        .route(
            "/api/management/session-rules/batch-service",
            post(session_rules::batch_update_service),
        )
        .route(
            "/api/management/session-rules/batch-provider",
            post(session_rules::batch_update_provider),
        )
        .route(
            "/api/management/session-rules/groups",
            get(session_rules::list_groups),
        )
        .route(
            "/api/management/session-rules/groups/upsert",
            post(session_rules::upsert_group),
        )
        .route(
            "/api/management/session-rules/groups/patch",
            post(session_rules::patch_group),
        )
        .route(
            "/api/management/session-rules/groups/delete",
            post(session_rules::delete_group),
        )
        .route(
            "/api/session/list-rule",
            get(session_rules::source_list_rules),
        )
        .route(
            "/api/session/update-rule",
            post(session_rules::source_update_rule),
        )
        .route(
            "/api/session/delete-rule",
            post(session_rules::source_delete_rule),
        )
        .route(
            "/api/session/batch-delete-rule",
            post(session_rules::source_batch_delete_rule),
        )
        .route(
            "/api/session/active-umos",
            get(session_rules::source_active_umos),
        )
        .route(
            "/api/session/list-all-with-status",
            get(session_rules::source_list_all_with_status),
        )
        .route(
            "/api/session/batch-update-service",
            post(session_rules::source_batch_update_service),
        )
        .route(
            "/api/session/batch-update-provider",
            post(session_rules::source_batch_update_provider),
        )
        .route(
            "/api/session/groups",
            get(session_rules::source_list_groups),
        )
        .route(
            "/api/session/group/create",
            post(session_rules::source_create_group),
        )
        .route(
            "/api/session/group/update",
            post(session_rules::source_update_group),
        )
        .route(
            "/api/session/group/delete",
            post(session_rules::source_delete_group),
        )
        .route(
            "/api/management/chat-projects/create",
            post(chat_projects::create),
        )
        .route(
            "/api/management/chat-projects/get",
            post(chat_projects::get),
        )
        .route(
            "/api/management/chat-projects/update",
            post(chat_projects::update),
        )
        .route(
            "/api/management/chat-projects/delete",
            post(chat_projects::delete),
        )
        .route(
            "/api/management/chat-projects/sessions/upsert",
            post(chat_projects::upsert_session),
        )
        .route(
            "/api/management/chat-projects/add-session",
            post(chat_projects::add_session),
        )
        .route(
            "/api/management/chat-projects/remove-session",
            post(chat_projects::remove_session),
        )
        .route(
            "/api/management/chat-projects/sessions",
            post(chat_projects::sessions),
        )
        .route(
            "/api/chatui_project/create",
            post(chat_projects::legacy_create),
        )
        .route("/api/chatui_project/list", get(chat_projects::legacy_list))
        .route("/api/chatui_project/get", get(chat_projects::legacy_get))
        .route(
            "/api/chatui_project/update",
            post(chat_projects::legacy_update),
        )
        .route(
            "/api/chatui_project/delete",
            get(chat_projects::legacy_delete),
        )
        .route(
            "/api/chatui_project/add_session",
            post(chat_projects::legacy_add_session),
        )
        .route(
            "/api/chatui_project/remove_session",
            post(chat_projects::legacy_remove_session),
        )
        .route(
            "/api/chatui_project/get_sessions",
            get(chat_projects::legacy_sessions),
        )
        .route("/api/management/tools", get(tools::catalog))
        .route("/api/management/tools/toggle", post(tools::toggle))
        .route("/api/management/commands", get(commands::catalog))
        .route("/api/management/commands/update", post(commands::update))
        .route("/api/management/mcp/servers", get(mcp::catalog))
        .route("/api/management/mcp/servers/upsert", post(mcp::upsert))
        .route("/api/management/mcp/servers/delete", post(mcp::delete))
        .route("/api/management/mcp/servers/check", post(mcp::check))
        .route("/api/management/mcp/servers/sync", post(mcp::sync))
        .route("/api/commands", get(commands::legacy_catalog))
        .route("/api/commands/conflicts", get(commands::legacy_conflicts))
        .route("/api/commands/toggle", post(commands::legacy_toggle))
        .route("/api/commands/rename", post(commands::legacy_rename))
        .route(
            "/api/commands/permission",
            post(commands::legacy_permission),
        )
        .route("/api/tools/list", get(tools::legacy_catalog))
        .route("/api/tools/toggle-tool", post(tools::legacy_toggle))
        .route("/api/tools/mcp/servers", get(mcp::legacy_catalog))
        .route("/api/tools/mcp/add", post(mcp::legacy_add))
        .route("/api/tools/mcp/update", post(mcp::legacy_update))
        .route("/api/tools/mcp/delete", post(mcp::legacy_delete))
        .route("/api/tools/mcp/test", post(mcp::legacy_check))
        .route(
            "/api/tools/mcp/sync-provider",
            post(mcp::legacy_sync_provider),
        )
        .route("/api/management/kb/catalog", get(knowledge_base::catalog))
        .route("/api/management/kb/create", post(knowledge_base::create))
        .route("/api/management/kb/get", post(knowledge_base::get))
        .route("/api/management/kb/update", post(knowledge_base::update))
        .route("/api/management/kb/delete", post(knowledge_base::delete))
        .route(
            "/api/management/kb/preflight",
            post(knowledge_base::preflight),
        )
        .route(
            "/api/management/kb/document/list",
            post(knowledge_base::list_documents),
        )
        .route(
            "/api/management/kb/document/get",
            post(knowledge_base::get_document),
        )
        .route(
            "/api/management/kb/document/delete",
            post(knowledge_base::delete_document),
        )
        .route(
            "/api/management/kb/chunk/list",
            post(knowledge_base::list_chunks),
        )
        .route(
            "/api/management/kb/chunk/delete",
            post(knowledge_base::delete_chunk),
        )
        .route(
            "/api/management/kb/retrieve",
            post(knowledge_base::retrieve),
        )
        .route("/api/management/kb/ingest", post(knowledge_base::ingest))
        .route(
            "/api/management/kb/upload/plan",
            post(knowledge_base::plan_upload),
        )
        .route(
            "/api/management/kb/upload/progress",
            post(knowledge_base::update_upload_progress),
        )
        .route(
            "/api/management/kb/upload/complete",
            post(knowledge_base::complete_upload),
        )
        .route(
            "/api/management/kb/upload/fail",
            post(knowledge_base::fail_upload),
        )
        .route(
            "/api/management/kb/upload/progress/{task_id}",
            get(knowledge_base::upload_progress),
        )
        .route("/api/kb/list", get(knowledge_base::legacy_list))
        .route("/api/kb/get", get(knowledge_base::legacy_get))
        .route("/api/kb/create", post(knowledge_base::legacy_create))
        .route("/api/kb/update", post(knowledge_base::legacy_update))
        .route("/api/kb/delete", post(knowledge_base::legacy_delete))
        .route("/api/kb/stats", get(knowledge_base::legacy_stats))
        .route(
            "/api/kb/document/list",
            get(knowledge_base::legacy_document_list),
        )
        .route(
            "/api/kb/document/upload",
            post(knowledge_base::legacy_document_upload),
        )
        .route(
            "/api/kb/document/import",
            post(knowledge_base::legacy_document_import),
        )
        .route(
            "/api/kb/document/get",
            get(knowledge_base::legacy_document_get),
        )
        .route(
            "/api/kb/document/delete",
            post(knowledge_base::legacy_document_delete),
        )
        .route("/api/kb/chunk/list", get(knowledge_base::legacy_chunk_list))
        .route(
            "/api/kb/chunk/delete",
            post(knowledge_base::legacy_chunk_delete),
        )
        .route("/api/kb/retrieve", post(knowledge_base::legacy_retrieve))
        .route(
            "/api/kb/document/upload/url",
            post(knowledge_base::legacy_document_upload_url),
        )
        .route(
            "/api/kb/document/upload/progress",
            get(knowledge_base::legacy_upload_progress),
        )
        .route("/api/management/update/check", get(update::check))
        .route("/api/management/update/releases", get(update::releases))
        .route("/api/management/update/changelog", get(update::changelog))
        .route("/api/update/check", get(update::legacy_update_check))
        .route("/api/update/releases", get(update::legacy_update_releases))
        .route("/api/update/do", post(update::legacy_update_project))
        .route(
            "/api/update/dashboard",
            post(update::legacy_update_dashboard),
        )
        .route(
            "/api/update/pip-install",
            post(update::legacy_update_package),
        )
        .route(
            "/api/update/migration",
            post(update::legacy_update_migration),
        )
        .route("/api/stat/get", get(update::legacy_stat_get))
        .route("/api/stat/version", get(update::legacy_stat_version))
        .route("/api/stat/start-time", get(update::legacy_stat_start_time))
        .route("/api/stat/restart-core", post(update::legacy_restart_core))
        .route(
            "/api/stat/test-ghproxy-connection",
            post(update::legacy_test_ghproxy_connection),
        )
        .route("/api/stat/changelog", get(update::legacy_changelog))
        .route(
            "/api/stat/changelog/list",
            get(update::legacy_changelog_list),
        )
        .route("/api/stat/first-notice", get(update::legacy_first_notice))
        .route(
            "/api/management/update/project-plan",
            post(update::project_plan),
        )
        .route(
            "/api/management/update/dashboard-plan",
            post(update::dashboard_plan),
        )
        .route(
            "/api/management/update/package-plan",
            post(update::package_plan),
        )
        .route(
            "/api/management/update/package-run",
            post(update::package_run),
        )
        .route(
            "/api/management/update/restart-plan",
            post(update::restart_plan),
        )
        .route(
            "/api/management/update/restart-run",
            post(update::restart_run),
        )
        .route(
            "/api/management/update/migration-check",
            get(update::migration_check),
        )
        .route(
            "/api/management/update/migration-plan",
            post(update::migration_plan),
        )
        .route(
            "/api/management/update/operations/{operation_id}",
            get(update::operation),
        )
        .route(
            "/api/management/update/operations",
            get(update::operation_catalog),
        )
        .route(
            "/api/management/update/operations/run",
            post(update::run_operation),
        )
        .route("/api/management/skills", get(skills::catalog))
        .route(
            "/api/management/skills/activation",
            post(skills::set_active),
        )
        .route(
            "/api/management/skills/install-plan",
            post(skills::install_plan),
        )
        .route("/api/management/skills/install", post(skills::install))
        .route(
            "/api/management/skills/delete-plan",
            post(skills::delete_plan),
        )
        .route("/api/management/skills/delete", post(skills::delete))
        .route("/api/skills", get(skills::legacy_catalog))
        .route("/api/skills/upload", post(skills::legacy_upload))
        .route(
            "/api/skills/batch-upload",
            post(skills::legacy_batch_upload),
        )
        .route("/api/skills/download", get(skills::legacy_download))
        .route("/api/skills/update", post(skills::legacy_update))
        .route("/api/skills/delete", post(skills::legacy_delete))
        .route(
            "/api/skills/neo/candidates",
            get(skills::legacy_neo_candidates),
        )
        .route("/api/skills/neo/releases", get(skills::legacy_neo_releases))
        .route("/api/skills/neo/payload", get(skills::legacy_neo_payload))
        .route("/api/skills/neo/evaluate", post(skills::legacy_neo_action))
        .route("/api/skills/neo/promote", post(skills::legacy_neo_action))
        .route("/api/skills/neo/rollback", post(skills::legacy_neo_action))
        .route("/api/skills/neo/sync", post(skills::legacy_neo_action))
        .route(
            "/api/skills/neo/delete-candidate",
            post(skills::legacy_neo_action),
        )
        .route(
            "/api/skills/neo/delete-release",
            post(skills::legacy_neo_action),
        )
        .route("/api/management/files/upload", post(files::upload))
        .route("/api/management/files/{token}", get(files::download))
        .route("/api/management/backup/precheck", post(backup::precheck))
        .route("/api/management/backup/export", post(backup::export))
        .route("/api/management/backup/import", post(backup::import))
        .route("/api/backup/list", get(backup::legacy_list))
        .route("/api/backup/export", post(backup::legacy_export))
        .route("/api/backup/upload", post(backup::legacy_upload))
        .route("/api/backup/upload/init", post(backup::legacy_upload_init))
        .route(
            "/api/backup/upload/chunk",
            post(backup::legacy_upload_chunk),
        )
        .route(
            "/api/backup/upload/complete",
            post(backup::legacy_upload_complete),
        )
        .route(
            "/api/backup/upload/abort",
            post(backup::legacy_upload_abort),
        )
        .route("/api/backup/check", post(backup::legacy_check))
        .route("/api/backup/import", post(backup::legacy_import))
        .route("/api/backup/progress", get(backup::legacy_progress))
        .route("/api/backup/download", get(backup::legacy_download))
        .route("/api/backup/delete", post(backup::legacy_delete))
        .route("/api/backup/rename", post(backup::legacy_rename))
        .route(
            "/api/management/backup/progress/{task_id}",
            get(backup::progress),
        )
        .route(
            "/api/management/backup/progress",
            get(backup::progress_catalog),
        )
        .route("/api/management/backup/files", get(backup::file_catalog))
        .route(
            "/api/management/backup/files/download",
            post(backup::file_download),
        )
        .route(
            "/api/management/backup/files/rename",
            post(backup::file_rename),
        )
        .route(
            "/api/management/backup/files/delete",
            post(backup::file_delete),
        )
        .route(
            "/api/management/backup/files/restore",
            post(backup::file_restore),
        )
        .route(
            "/api/management/backup/upload/start",
            post(backup::upload_start),
        )
        .route(
            "/api/management/backup/upload/chunk",
            post(backup::upload_chunk),
        )
        .route(
            "/api/management/backup/upload/complete",
            post(backup::upload_complete),
        )
        .route(
            "/api/management/backup/upload/abort",
            post(backup::upload_abort),
        )
        .route(
            "/api/management/plugin-market/install-plan",
            post(plugin_market::install_plan),
        )
        .route(
            "/api/management/plugin-market/install",
            post(plugin_market::install),
        )
        .route(
            "/api/management/plugin-market/update-plan",
            post(plugin_market::update_plan),
        )
        .route(
            "/api/management/plugin-market/update",
            post(plugin_market::update),
        )
        .route(
            "/api/management/plugin-market/uninstall-plan",
            post(plugin_market::uninstall_plan),
        )
        .route(
            "/api/management/plugin-market/uninstall",
            post(plugin_market::uninstall),
        )
        .route(
            "/api/management/plugin-market/update-all-plan",
            get(plugin_market::update_all_plan),
        )
        .route(
            "/api/management/plugin-market/update-all",
            post(plugin_market::update_all),
        )
}

async fn status(State(state): State<ManagementApiState>) -> Json<ManagementStatusResponse> {
    Json(state.status())
}

async fn providers(State(state): State<ManagementApiState>) -> Json<ProviderManagementResponse> {
    Json(state.providers().clone())
}

async fn platforms(State(state): State<ManagementApiState>) -> Json<PlatformManagementResponse> {
    Json(state.platforms().clone())
}

async fn plugins(State(state): State<ManagementApiState>) -> Json<PluginManagementResponse> {
    Json(state.plugins().clone())
}
