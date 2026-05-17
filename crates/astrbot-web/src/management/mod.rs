mod api_key;
mod auth;
mod backup;
mod chat_projects;
mod config;
mod files;
mod knowledge_base;
mod platforms;
mod plugin_market;
mod plugins;
mod providers;
mod session_rules;
mod skills;
mod status;
mod tools;
mod update;

use astrbot_platform::PlatformManager;
use astrbot_plugin::PluginRegistry;
use astrbot_provider::ProviderManager;
use astrbot_runtime::RuntimeConfigService;
use axum::{
    Json, Router,
    extract::State,
    middleware,
    routing::{get, post},
};

pub use api_key::{
    ApiKeyAuthDecision, ApiKeyIssuer, ApiKeyRejectionReason, IssuedApiKey, OpenApiScope,
    OpenApiScopeSet, PresentedApiKey, authorize_api_key, extract_presented_api_key, hash_api_key,
};
pub use auth::{
    AuthRejectionReason, DashboardAuthDecision, DashboardAuthPolicy, ManagementAuthState,
    extract_bearer_token,
};
pub use backup::{
    ManagementBackupAbortRequest, ManagementBackupAbortResponse, ManagementBackupChunkRequest,
    ManagementBackupChunkResponse, ManagementBackupCompleteRequest,
    ManagementBackupCompleteResponse, ManagementBackupExportRequest, ManagementBackupImportRequest,
    ManagementBackupJobResponse, ManagementBackupPrecheckRequest, ManagementBackupPrecheckResponse,
    ManagementBackupProgressResponse, ManagementBackupState, ManagementBackupUploadStartRequest,
    ManagementBackupUploadStartResponse,
};
pub use chat_projects::{
    ManagementChatProjectActorRequest, ManagementChatProjectCatalogResponse,
    ManagementChatProjectCreateRequest, ManagementChatProjectDescriptor,
    ManagementChatProjectGetRequest, ManagementChatProjectMembershipRequest,
    ManagementChatProjectMutationResponse, ManagementChatProjectResponse,
    ManagementChatProjectSessionsResponse, ManagementChatProjectState,
    ManagementChatProjectUpdateRequest, ManagementPlatformSessionDescriptor,
};
pub use config::{
    ManagementConfigMutationRequest, ManagementConfigMutationResponse,
    ManagementConfigSchemaResponse,
};
pub use files::{ManagementFileDownloadState, ScopedDownloadError, ScopedDownloadFile};
pub use knowledge_base::{
    ManagementKnowledgeBaseCatalogResponse, ManagementKnowledgeBaseCreateRequest,
    ManagementKnowledgeBaseIdRequest, ManagementKnowledgeBaseResponse,
    ManagementKnowledgeBaseState, ManagementKnowledgeBaseUpdateRequest,
    ManagementKnowledgeChunkCatalogResponse, ManagementKnowledgeChunkDeleteRequest,
    ManagementKnowledgeDocumentCatalogResponse, ManagementKnowledgeDocumentIdRequest,
    ManagementKnowledgeMutationResponse, ManagementKnowledgePreflightResponse,
    ManagementKnowledgeProviderPreflightRequest, ManagementKnowledgeUploadCompleteRequest,
    ManagementKnowledgeUploadFailRequest, ManagementKnowledgeUploadPlanRequest,
    ManagementKnowledgeUploadProgressRequest, ManagementKnowledgeUploadTaskResponse,
};
pub use platforms::PlatformManagementResponse;
pub use plugin_market::{
    PluginMarketCatalogResponse, PluginMarketManagementState, PluginMarketPlanRequest,
    PluginMarketPlanResponse,
};
pub use plugins::{PluginHandlerManagementResponse, PluginManagementResponse};
pub use providers::ProviderManagementResponse;
pub use session_rules::ManagementSessionRuleState;
pub use skills::{
    ManagementSkillActivationRequest, ManagementSkillActivationResponse,
    ManagementSkillCatalogResponse, ManagementSkillDeletePlanRequest,
    ManagementSkillDeletePlanResponse, ManagementSkillDescriptor,
    ManagementSkillInstallPlanRequest, ManagementSkillInstallPlanResponse, ManagementSkillState,
};
pub use status::ManagementStatusResponse;
pub use tools::{
    ManagementToolCatalogResponse, ManagementToolDescriptor, ManagementToolState,
    ManagementToolToggleRequest, ManagementToolToggleResponse,
};
pub use update::{
    DashboardUpdatePlanRequest, MaintenanceCheckResponse, MaintenanceMigrationCheckResponse,
    MaintenanceOperationResponse, MaintenancePackagePlanResponse, ManagementMaintenanceState,
    ProjectUpdatePlanRequest,
};

#[derive(Clone, Debug)]
pub struct ManagementApiState {
    providers: ProviderManagementResponse,
    platforms: PlatformManagementResponse,
    plugins: PluginManagementResponse,
    config_service: Option<RuntimeConfigService>,
    plugin_market: Option<PluginMarketManagementState>,
    file_downloads: Option<ManagementFileDownloadState>,
    backup: Option<ManagementBackupState>,
    chat_projects: Option<ManagementChatProjectState>,
    session_rules: Option<ManagementSessionRuleState>,
    skills: Option<ManagementSkillState>,
    tools: Option<ManagementToolState>,
    maintenance: Option<ManagementMaintenanceState>,
    knowledge_base: Option<ManagementKnowledgeBaseState>,
}

impl ManagementApiState {
    pub fn new(
        providers: ProviderManagementResponse,
        platforms: PlatformManagementResponse,
        plugins: PluginManagementResponse,
    ) -> Self {
        Self {
            providers,
            platforms,
            plugins,
            config_service: None,
            plugin_market: None,
            file_downloads: None,
            backup: None,
            chat_projects: None,
            session_rules: None,
            skills: None,
            tools: None,
            maintenance: None,
            knowledge_base: None,
        }
    }

    pub fn with_config_service(mut self, config_service: RuntimeConfigService) -> Self {
        self.config_service = Some(config_service);
        self
    }

    pub fn with_plugin_market(mut self, plugin_market: PluginMarketManagementState) -> Self {
        self.plugin_market = Some(plugin_market);
        self
    }

    pub fn with_file_downloads(mut self, file_downloads: ManagementFileDownloadState) -> Self {
        self.file_downloads = Some(file_downloads);
        self
    }

    pub fn with_backup(mut self, backup: ManagementBackupState) -> Self {
        self.backup = Some(backup);
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

    pub fn with_maintenance(mut self, maintenance: ManagementMaintenanceState) -> Self {
        self.maintenance = Some(maintenance);
        self
    }

    pub fn with_knowledge_base(mut self, knowledge_base: ManagementKnowledgeBaseState) -> Self {
        self.knowledge_base = Some(knowledge_base);
        self
    }

    pub fn from_managers(
        provider_manager: &ProviderManager,
        platform_manager: &PlatformManager,
        plugin_registry: &PluginRegistry,
    ) -> Self {
        Self::new(
            ProviderManagementResponse::from_manager(provider_manager),
            PlatformManagementResponse::from_manager(platform_manager),
            PluginManagementResponse::from_registry(plugin_registry),
        )
    }

    pub fn providers(&self) -> &ProviderManagementResponse {
        &self.providers
    }

    pub fn platforms(&self) -> &PlatformManagementResponse {
        &self.platforms
    }

    pub fn plugins(&self) -> &PluginManagementResponse {
        &self.plugins
    }

    pub fn config_service(&self) -> Option<&RuntimeConfigService> {
        self.config_service.as_ref()
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

    pub fn maintenance(&self) -> Option<&ManagementMaintenanceState> {
        self.maintenance.as_ref()
    }

    pub fn knowledge_base(&self) -> Option<&ManagementKnowledgeBaseState> {
        self.knowledge_base.as_ref()
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
    management_routes()
        .route_layer(middleware::from_fn_with_state(
            auth,
            auth::require_management_auth,
        ))
        .with_state(state)
}

fn management_routes() -> Router<ManagementApiState> {
    Router::new()
        .route("/api/management/status", get(status))
        .route("/api/management/providers", get(providers))
        .route("/api/management/platforms", get(platforms))
        .route("/api/management/plugins", get(plugins))
        .route("/api/management/config/schema", get(config::schema))
        .route(
            "/api/management/config/preview",
            post(config::preview_update),
        )
        .route("/api/management/config/apply", post(config::apply_update))
        .route("/api/management/plugin-market", get(plugin_market::catalog))
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
        .route("/api/management/tools", get(tools::catalog))
        .route("/api/management/tools/toggle", post(tools::toggle))
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
        .route("/api/management/update/check", get(update::check))
        .route("/api/management/update/releases", get(update::releases))
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
        .route("/api/management/skills", get(skills::catalog))
        .route(
            "/api/management/skills/activation",
            post(skills::set_active),
        )
        .route(
            "/api/management/skills/install-plan",
            post(skills::install_plan),
        )
        .route(
            "/api/management/skills/delete-plan",
            post(skills::delete_plan),
        )
        .route("/api/management/files/{token}", get(files::download))
        .route("/api/management/backup/precheck", post(backup::precheck))
        .route("/api/management/backup/export", post(backup::export))
        .route("/api/management/backup/import", post(backup::import))
        .route(
            "/api/management/backup/progress/{task_id}",
            get(backup::progress),
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
            "/api/management/plugin-market/update-plan",
            post(plugin_market::update_plan),
        )
        .route(
            "/api/management/plugin-market/uninstall-plan",
            post(plugin_market::uninstall_plan),
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
