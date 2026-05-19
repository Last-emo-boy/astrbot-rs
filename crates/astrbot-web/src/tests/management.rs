use std::{
    fs,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use astrbot_agent::{ResolvedSubagent, SubagentConfig, SubagentConfigSource};
use astrbot_conversation::{ChatProjectService, ConversationRecord, ConversationService};
use astrbot_core::{MessageChain, MessageEvent, Result};
use astrbot_cron::{
    ActiveAgentCronPayload, CronJob, CronJobSchedule, CronScheduler, DueCronScheduleDriver,
    InMemoryCronJobRepository, ProactiveAgentWakeService, RecordingCronEventSink,
};
use astrbot_kb::{
    ChunkId, DocumentId, EmbeddedKnowledgeChunk, InMemoryKnowledgeBaseManagementStore,
    InMemoryKnowledgeDocumentRepository, InMemoryKnowledgeMediaStore,
    InMemoryKnowledgeUploadTaskStore, InMemoryVectorStore, KnowledgeBaseCreateCommand,
    KnowledgeBaseId, KnowledgeBaseManagementService, KnowledgeChunk, KnowledgeDocument,
    KnowledgeProviderPreflightService, KnowledgeUploadTaskService, VectorStore,
};
use astrbot_maintenance::{
    DashboardUpdatePlan, MaintenanceMigrationCheck, MaintenanceMigrationRequest,
    MaintenancePackageInstallPlan, ProjectUpdatePlan, ReleaseMetadata,
    RuntimeConfigMigrationDescriptor, SqliteMaintenanceOperationStore,
};
use astrbot_metrics::{MetricEvent, UsageRecord};
use astrbot_observability::{InMemoryLogBuffer, LogEntry, LogLevel, LogSource, TraceEvent};
use astrbot_persona::{InMemoryPersonaRepository, PersonaFolder, PersonaManager, PersonaProfile};
use astrbot_platform::{
    AIOCQHTTP_PLATFORM_TYPE, CONSOLE_PLATFORM_TYPE, PlatformBuildContext, PlatformConfig,
    PlatformManager, PlatformRegistry,
};
use astrbot_plugin::{
    HandlerMetadata, PluginCompatibility, PluginControl, PluginEventType, PluginHandler,
    PluginInstallSource, PluginLifecycleState, PluginLoadSource, PluginManifest, PluginMarketEntry,
    PluginPackageDescriptor, PluginRegistry, RegisteredHandler,
};
use astrbot_provider::{
    ChatProviderConfig, EmbeddingProviderConfig, OPENAI_CHAT_PROVIDER_TYPE, ProviderManager,
    ProviderManagerConfigSet, ProviderRegistry, RerankProviderConfig,
};
use astrbot_runtime::{
    REDACTED_SECRET, RuntimeChatProviderConfig, RuntimeCommandPluginConfig, RuntimeConfig,
    RuntimeConfigReloadAction, RuntimeConfigService, RuntimePlatformConfig,
    RuntimeProviderSourceConfig, UmopConfigRoute, UmopConfigRouter,
};
use astrbot_session::ProviderCapability;
use astrbot_skill::{
    SkillActivationPolicy, SkillCatalog, SkillDescriptor, SkillPromptRenderer, SkillPromptRuntime,
    SkillSandboxCache, SkillSandboxEntry, SkillSource,
};
use astrbot_storage::{
    ApiKeyRecord, ApiKeyRepository, AttachmentRepository, BACKUP_UPLOAD_CHUNK_SIZE,
    BackupExportJobRequest, BackupExportPackage, BackupExportPort, BackupExportRequest,
    BackupImportJobRequest, BackupImportMode, BackupImportPort, BackupImportPrecheck,
    BackupImportResult, BackupJobService, BackupManifest, BackupRepositoryPort, BackupTableDump,
    ChatProjectRepository, FileTokenRecord, FileTokenRepository, FileTokenScope,
    FilesystemBackupExporter, InMemoryApiKeyRepository, InMemoryChatProjectRepository,
    InMemoryFileTokenRepository, InMemorySessionRuleRepository, PlatformSessionRecord,
    PlatformStatsRecord, PlatformStatsRepository, SqliteBackupImporter, SqliteBackupRepository,
    SqliteJsonStore, SqliteStorage, verify_backup_archive,
};
use astrbot_tool::{ToolCatalog, ToolDescriptor, ToolSource, ToolSourceMetadata};
use async_trait::async_trait;
use axum::{
    body::to_bytes,
    http::{StatusCode, header::CONTENT_TYPE},
};
use serde_json::json;
use sha1::{Digest, Sha1};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::{
    DashboardAuthPolicy, DashboardCapabilitiesResponse, DashboardClosureLevel,
    LocalMaintenanceExecutor, MaintenanceMigrationExecutor, MaintenancePackageExecutor,
    MaintenanceReleaseExecutor, MaintenanceRestartExecutor, MaintenanceRestartRequest,
    ManagementApiKeyState, ManagementApiState, ManagementAuthState, ManagementBackupState,
    ManagementChatProjectState, ManagementConfigApplyExecution,
    ManagementConfigApplyExecutionRequest, ManagementConfigApplyExecutor,
    ManagementConfigApplyFuture, ManagementConfigApplyState, ManagementConfigRouteState,
    ManagementConversationState, ManagementCronState, ManagementFileDownloadState,
    ManagementKnowledgeBaseState, ManagementMaintenanceState, ManagementMcpState,
    ManagementObservabilityState, ManagementPersonaState, ManagementPluginLifecycleState,
    ManagementPluginSeed, ManagementSessionRuleState, ManagementSkillState,
    ManagementStatusResponse, ManagementSubagentConfig, ManagementSubagentExecuteRequest,
    ManagementSubagentExecutionBridge, ManagementSubagentExecutionResult, ManagementSubagentState,
    ManagementToolState, PluginMarketManagementState, hash_api_key, management_router,
    management_router_with_auth,
};

use crate::management::{
    ManagementPlatformHealthCheck, ManagementPlatformHealthFuture, ManagementPlatformHealthResult,
    ManagementProviderHealthCheck, ManagementProviderHealthFuture, ManagementProviderHealthResult,
    ManagementProviderModelsFuture, ManagementProviderModelsResult,
};

use super::support::{
    delete, get, get_with_bearer, patch_json, post_json, post_json_with_bearer,
    post_json_with_headers, post_multipart, put_json, response_json,
};

#[tokio::test]
async fn management_status_reads_provider_platform_and_plugin_facades() {
    let router = management_router(management_state_fixture());

    let response = get(router, "/api/management/status").await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: ManagementStatusResponse = response_json(response).await;
    assert_eq!(payload.providers.chat_provider_count, 1);
    assert_eq!(
        payload.providers.default_chat_provider_id.as_deref(),
        Some("mock-provider")
    );
    assert_eq!(payload.platforms.platform_count, 2);
    assert_eq!(
        payload.platforms.platform_ids,
        vec!["mock-platform".to_string(), "webchat".to_string()]
    );
    assert_eq!(payload.plugins.handler_count, 1);
    assert_eq!(payload.plugins.handlers[0].plugin_name, "builtin");
    assert_eq!(payload.plugins.handlers[0].handler_name, "ping");
}

#[tokio::test]
async fn management_sqlite_storage_defaults_persist_after_state_rebuild() {
    let db_path = temp_management_file_path("sqlite-defaults.db");
    cleanup_sqlite_files(&db_path);

    let router = management_router(
        management_state_fixture()
            .with_sqlite_storage_path(&db_path)
            .expect("sqlite-backed management state should build"),
    );

    let issue_response = post_json(
        router.clone(),
        "/api/management/api-keys/issue",
        json!({
            "key_id": "key-1",
            "name": "Dashboard",
            "secret": "ak_sqlite_secret",
            "scopes": ["management.read"],
            "created_by": "admin"
        }),
    )
    .await;
    assert_eq!(issue_response.status(), StatusCode::OK);

    let conversation_response = post_json(
        router.clone(),
        "/api/management/conversations/upsert",
        json!({
            "platform_id": "webchat",
            "conversation_id": "conversation-1",
            "title": "General",
            "persona_id": "default",
            "set_current": true
        }),
    )
    .await;
    assert_eq!(conversation_response.status(), StatusCode::OK);

    let session_rule_response = post_json(
        router.clone(),
        "/api/management/session-rules/update",
        json!({
            "umo": "webchat:private:alice",
            "key": { "type": "service" },
            "value": {
                "kind": "service",
                "session_enabled": true,
                "llm_enabled": false
            }
        }),
    )
    .await;
    assert_eq!(session_rule_response.status(), StatusCode::OK);

    let project_response = post_json(
        router.clone(),
        "/api/management/chat-projects/create",
        json!({
            "creator": "alice",
            "title": "Research",
            "now": "2026-05-18T00:00:00Z"
        }),
    )
    .await;
    assert_eq!(project_response.status(), StatusCode::OK);
    let project: serde_json::Value = response_json(project_response).await;
    assert_eq!(project["project"]["project_id"], "project-1");

    let persona_response = post_json(
        router.clone(),
        "/api/management/personas/upsert",
        json!({
            "id": "analyst",
            "system_prompt": "be rigorous",
            "sort_order": 1
        }),
    )
    .await;
    assert_eq!(persona_response.status(), StatusCode::OK);

    let kb_response = post_json(
        router.clone(),
        "/api/management/kb/create",
        json!({
            "kb_id": "kb-1",
            "name": "Docs",
            "embedding_provider_id": "embedding"
        }),
    )
    .await;
    assert_eq!(kb_response.status(), StatusCode::OK);
    let kb_ingest_response = post_json(
        router.clone(),
        "/api/management/kb/ingest",
        json!({
            "kb_id": "kb-1",
            "doc_id": "doc-1",
            "name": "Intro",
            "source_kind": "text",
            "content": "Dashboard knowledge survives sqlite reload."
        }),
    )
    .await;
    assert_eq!(kb_ingest_response.status(), StatusCode::OK);

    let cron_response = post_json(
        router,
        "/api/management/cron/jobs/upsert",
        json!({
            "job": CronJob::active_agent(
                "daily-default",
                "Daily Default",
                CronJobSchedule::cron("0 8 * * *"),
                ActiveAgentCronPayload::new("webchat:conversation-1", "check status")
            )
        }),
    )
    .await;
    assert_eq!(cron_response.status(), StatusCode::OK);

    let reloaded = management_router(
        management_state_fixture()
            .with_sqlite_storage_path(&db_path)
            .expect("sqlite-backed management state should rebuild"),
    );

    let api_keys_response = get(reloaded.clone(), "/api/management/api-keys").await;
    assert_eq!(api_keys_response.status(), StatusCode::OK);
    let api_keys: serde_json::Value = response_json(api_keys_response).await;
    assert!(
        api_keys["api_keys"]
            .as_array()
            .expect("api key catalog")
            .iter()
            .any(|key| key["key_id"] == "key-1")
    );

    let conversations_response = post_json(
        reloaded.clone(),
        "/api/management/conversations",
        json!({ "platform_id": "webchat" }),
    )
    .await;
    assert_eq!(conversations_response.status(), StatusCode::OK);
    let conversations: serde_json::Value = response_json(conversations_response).await;
    assert!(
        conversations["conversations"]
            .as_array()
            .expect("conversation catalog")
            .iter()
            .any(
                |conversation| conversation["conversation_id"] == "conversation-1"
                    && conversation["current"] == true
            )
    );

    let rules_response = get(reloaded.clone(), "/api/management/session-rules").await;
    assert_eq!(rules_response.status(), StatusCode::OK);
    let rules: serde_json::Value = response_json(rules_response).await;
    assert!(
        rules["rules"]
            .as_array()
            .expect("session rules")
            .iter()
            .any(|rule| rule["umo"] == "webchat:private:alice")
    );

    let projects_response = post_json(
        reloaded.clone(),
        "/api/management/chat-projects",
        json!({ "actor": "alice" }),
    )
    .await;
    assert_eq!(projects_response.status(), StatusCode::OK);
    let projects: serde_json::Value = response_json(projects_response).await;
    assert!(
        projects["projects"]
            .as_array()
            .expect("project catalog")
            .iter()
            .any(|project| project["project_id"] == "project-1" && project["title"] == "Research")
    );

    let personas_response =
        post_json(reloaded.clone(), "/api/management/personas", json!({})).await;
    assert_eq!(personas_response.status(), StatusCode::OK);
    let personas: serde_json::Value = response_json(personas_response).await;
    assert!(
        personas["personas"]
            .as_array()
            .expect("persona catalog")
            .iter()
            .any(|persona| persona["id"] == "analyst" && persona["system_prompt"] == "be rigorous")
    );

    let kb_catalog_response = get(reloaded.clone(), "/api/management/kb/catalog").await;
    assert_eq!(kb_catalog_response.status(), StatusCode::OK);
    let kb_catalog: serde_json::Value = response_json(kb_catalog_response).await;
    assert!(
        kb_catalog["knowledge_bases"]
            .as_array()
            .expect("knowledge base catalog")
            .iter()
            .any(|kb| kb["kb_id"] == "kb-1" && kb["name"] == "Docs")
    );
    let kb_retrieve_response = post_json(
        reloaded.clone(),
        "/api/management/kb/retrieve",
        json!({
            "query": "sqlite reload",
            "kb_ids": ["kb-1"],
            "top_k": 1
        }),
    )
    .await;
    assert_eq!(kb_retrieve_response.status(), StatusCode::OK);
    let kb_retrieved: serde_json::Value = response_json(kb_retrieve_response).await;
    assert_eq!(kb_retrieved["mode"], "hybrid_vector");
    assert_eq!(kb_retrieved["results"][0]["doc_id"], "doc-1");

    let cron_list_response = post_json(
        reloaded,
        "/api/management/cron/jobs",
        json!({ "kind": "active_agent" }),
    )
    .await;
    assert_eq!(cron_list_response.status(), StatusCode::OK);
    let cron_list: serde_json::Value = response_json(cron_list_response).await;
    assert!(
        cron_list["jobs"]
            .as_array()
            .expect("cron jobs")
            .iter()
            .any(|job| job["job_id"] == "daily-default" && job["persistent"] == true)
    );

    cleanup_sqlite_files(&db_path);
}

#[tokio::test]
async fn source_compatible_management_object_routes_persist_after_sqlite_rebuild() {
    let db_path = temp_management_file_path("source-object-facades.db");
    cleanup_sqlite_files(&db_path);

    let router = management_router(
        management_state_fixture()
            .with_sqlite_storage_path(&db_path)
            .expect("sqlite-backed management state should build"),
    );

    let folder_response = post_json(
        router.clone(),
        "/api/persona/folder/create",
        json!({
            "folder_id": "ops-folder",
            "name": "Ops",
            "description": "Operations personas",
            "sort_order": 2
        }),
    )
    .await;
    assert_eq!(folder_response.status(), StatusCode::OK);
    let folder: serde_json::Value = response_json(folder_response).await;
    assert_eq!(folder["status"], "ok");
    assert_eq!(folder["data"]["folder"]["folder_id"], "ops-folder");

    let persona_response = post_json(
        router.clone(),
        "/api/persona/create",
        json!({
            "persona_id": "ops",
            "system_prompt": "Operate carefully",
            "folder_id": "ops-folder",
            "tools": ["diagnostics"],
            "skills": ["runbook"],
            "sort_order": 1
        }),
    )
    .await;
    assert_eq!(persona_response.status(), StatusCode::OK);
    let persona: serde_json::Value = response_json(persona_response).await;
    assert_eq!(persona["data"]["persona"]["persona_id"], "ops");

    let project_response = post_json(
        router.clone(),
        "/api/chatui_project/create",
        json!({
            "title": "Ops Project",
            "emoji": "!",
            "description": "Incident sessions"
        }),
    )
    .await;
    assert_eq!(project_response.status(), StatusCode::OK);
    let project: serde_json::Value = response_json(project_response).await;
    let project_id = project["data"]["project_id"]
        .as_str()
        .expect("project id")
        .to_string();

    let session_response = post_json(
        router.clone(),
        "/api/management/chat-projects/sessions/upsert",
        json!({
            "session_id": "session-ops",
            "platform_id": "webchat",
            "creator": "guest",
            "display_name": "Ops Room",
            "is_group": true,
            "now": "2026-05-19T00:00:00Z"
        }),
    )
    .await;
    assert_eq!(session_response.status(), StatusCode::OK);

    let add_session_response = post_json(
        router.clone(),
        "/api/chatui_project/add_session",
        json!({
            "project_id": project_id,
            "session_id": "session-ops"
        }),
    )
    .await;
    assert_eq!(add_session_response.status(), StatusCode::OK);

    let cron_response = post_json(
        router,
        "/api/cron/jobs",
        json!({
            "name": "Ops Wake",
            "session": "webchat:session-ops:group",
            "note": "Check incident status",
            "cron_expression": "0 9 * * *",
            "timezone": "Asia/Shanghai"
        }),
    )
    .await;
    assert_eq!(cron_response.status(), StatusCode::OK);
    let cron: serde_json::Value = response_json(cron_response).await;
    let cron_job_id = cron["data"]["job_id"]
        .as_str()
        .expect("cron job id")
        .to_string();

    let reloaded = management_router(
        management_state_fixture()
            .with_sqlite_storage_path(&db_path)
            .expect("sqlite-backed management state should rebuild"),
    );

    let personas_response = get(reloaded.clone(), "/api/persona/list?folder_id=ops-folder").await;
    assert_eq!(personas_response.status(), StatusCode::OK);
    let personas: serde_json::Value = response_json(personas_response).await;
    assert_eq!(personas["status"], "ok");
    assert!(
        personas["data"]
            .as_array()
            .expect("personas")
            .iter()
            .any(|persona| persona["persona_id"] == "ops" && persona["folder_id"] == "ops-folder")
    );

    let folders_response = get(reloaded.clone(), "/api/persona/folder/list").await;
    assert_eq!(folders_response.status(), StatusCode::OK);
    let folders: serde_json::Value = response_json(folders_response).await;
    assert!(
        folders["data"]
            .as_array()
            .expect("folders")
            .iter()
            .any(|folder| folder["folder_id"] == "ops-folder" && folder["name"] == "Ops")
    );

    let projects_response = get(reloaded.clone(), "/api/chatui_project/list").await;
    assert_eq!(projects_response.status(), StatusCode::OK);
    let projects: serde_json::Value = response_json(projects_response).await;
    assert!(
        projects["data"]
            .as_array()
            .expect("projects")
            .iter()
            .any(|project| project["project_id"] == project_id
                && project["title"] == "Ops Project")
    );

    let sessions_response = get(
        reloaded.clone(),
        &format!("/api/chatui_project/get_sessions?project_id={project_id}"),
    )
    .await;
    assert_eq!(sessions_response.status(), StatusCode::OK);
    let sessions: serde_json::Value = response_json(sessions_response).await;
    assert!(
        sessions["data"]
            .as_array()
            .expect("project sessions")
            .iter()
            .any(|session| session["session_id"] == "session-ops"
                && session["display_name"] == "Ops Room")
    );

    let cron_list_response = get(reloaded, "/api/cron/jobs?type=active_agent").await;
    assert_eq!(cron_list_response.status(), StatusCode::OK);
    let cron_list: serde_json::Value = response_json(cron_list_response).await;
    assert!(
        cron_list["data"]
            .as_array()
            .expect("cron jobs")
            .iter()
            .any(|job| job["job_id"] == cron_job_id
                && job["cron_expression"] == "0 9 * * *"
                && job["note"] == "Check incident status")
    );

    cleanup_sqlite_files(&db_path);
}

#[tokio::test]
async fn management_provider_routes_mutate_runtime_config_and_validate_templates() {
    let path = temp_management_config_path("provider-crud");
    let _ = std::fs::remove_file(&path);
    let state = management_state_fixture()
        .with_config_service(RuntimeConfigService::new(&path))
        .with_provider_health_check(Arc::new(StaticProviderHealthCheck::available()));
    let router = management_router(state);

    let catalog_response = get(router.clone(), "/api/management/providers/catalog").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert!(
        catalog["templates"]
            .as_array()
            .expect("templates")
            .iter()
            .any(|template| template["provider_type"] == OPENAI_CHAT_PROVIDER_TYPE)
    );

    let upsert_response = post_json(
        router.clone(),
        "/api/management/providers/upsert",
        json!({
            "provider": {
                "id": "dash-openai",
                "type": OPENAI_CHAT_PROVIDER_TYPE,
                "enabled": true,
                "model": "chat-model",
                "api_base": "https://example.invalid/v1",
                "api_key": "sk-dashboard",
                "timeout_secs": 30
            },
            "set_default": true
        }),
    )
    .await;
    assert_eq!(upsert_response.status(), StatusCode::OK);
    let upserted: serde_json::Value = response_json(upsert_response).await;
    assert_eq!(
        upserted["catalog"]["default_chat_provider_id"],
        "dash-openai"
    );
    assert!(
        upserted["catalog"]["chat_providers"]
            .as_array()
            .expect("providers")
            .iter()
            .any(|provider| provider["id"] == "dash-openai"
                && provider["api_key_configured"] == true)
    );
    let saved = RuntimeConfig::from_json_file(&path).expect("config should persist provider");
    assert_eq!(saved.default_chat_provider_id, "dash-openai");
    assert_eq!(
        saved
            .chat_providers
            .iter()
            .find(|provider| provider.id == "dash-openai")
            .and_then(|provider| provider.api_key.as_deref()),
        Some("sk-dashboard")
    );

    let check_response = post_json(
        router.clone(),
        "/api/management/providers/check",
        json!({ "id": "dash-openai" }),
    )
    .await;
    assert_eq!(check_response.status(), StatusCode::OK);
    let check: serde_json::Value = response_json(check_response).await;
    assert_eq!(check["ok"], true);
    assert_eq!(check["status"], "available");
    assert!(
        check["message"]
            .as_str()
            .expect("provider check message")
            .contains("lightweight chat request")
    );

    let models_response = post_json(
        router.clone(),
        "/api/management/providers/models",
        json!({ "provider_type": OPENAI_CHAT_PROVIDER_TYPE }),
    )
    .await;
    assert_eq!(models_response.status(), StatusCode::OK);
    let models: serde_json::Value = response_json(models_response).await;
    assert!(models["models"].is_array());
    assert_eq!(models["dynamic"], false);

    let delete_response = post_json(
        router,
        "/api/management/providers/delete",
        json!({ "id": "dash-openai" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_response).await;
    assert_eq!(deleted["changed"], true);
    let saved = RuntimeConfig::from_json_file(&path).expect("config should persist delete");
    assert!(
        !saved
            .chat_providers
            .iter()
            .any(|provider| provider.id == "dash-openai")
    );

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn source_compatible_provider_facades_match_provider_page_contract() {
    let path = temp_management_config_path("provider-source-facade");
    let _ = std::fs::remove_file(&path);
    let state = management_state_fixture()
        .with_config_service(RuntimeConfigService::new(&path))
        .with_provider_health_check(Arc::new(StaticProviderHealthCheck::available()));
    let router = management_router(state);

    let template_response = get(router.clone(), "/api/config/provider/template").await;
    assert_eq!(template_response.status(), StatusCode::OK);
    let template: serde_json::Value = response_json(template_response).await;
    assert_eq!(template["status"], "ok");
    assert_eq!(
        template["data"]["config_schema"]["provider"]["config_template"]["OpenAI"]["provider_type"],
        "chat_completion"
    );

    let models_response = get(
        router.clone(),
        "/api/config/provider_sources/models?source_id=openai",
    )
    .await;
    assert_eq!(models_response.status(), StatusCode::OK);
    let models: serde_json::Value = response_json(models_response).await;
    assert!(
        models["data"]["models"]
            .as_array()
            .expect("models")
            .iter()
            .any(|model| model == "gpt-4.1-mini")
    );
    assert!(
        models["data"]["model_metadata"]["gpt-4.1-mini"]["tool_call"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(models["data"]["dynamic"], false);

    let source_create_response = post_json(
        router.clone(),
        "/api/config/provider_sources/update",
        json!({
            "original_id": "source-a",
            "config": {
                "id": "source-a",
                "type": OPENAI_CHAT_PROVIDER_TYPE,
                "provider": "openai",
                "api_base": "https://api.example/v1",
                "key": "sk-source",
                "timeout_secs": 45
            }
        }),
    )
    .await;
    assert_eq!(source_create_response.status(), StatusCode::OK);
    let saved =
        RuntimeConfig::from_json_file(&path).expect("config should persist provider source");
    assert_eq!(saved.provider_sources.len(), 1);
    assert_eq!(
        saved.provider_sources[0].api_key.as_deref(),
        Some("sk-source")
    );

    let create_response = post_json(
        router.clone(),
        "/api/config/provider/new",
        json!({
            "id": "source-a/gpt-4.1-mini",
            "provider_source_id": "source-a",
            "enable": false,
            "model": "gpt-4.1-mini",
            "modalities": ["text", "image", "tool_use"],
            "max_context_tokens": 1048576
        }),
    )
    .await;
    assert_eq!(create_response.status(), StatusCode::OK);
    let saved = RuntimeConfig::from_json_file(&path).expect("config should persist provider");
    let provider = saved
        .chat_providers
        .iter()
        .find(|provider| provider.id == "source-a/gpt-4.1-mini")
        .expect("source model provider");
    assert_eq!(provider.provider_source_id.as_deref(), Some("source-a"));
    assert_eq!(provider.api_key.as_deref(), Some("sk-source"));
    assert_eq!(provider.api_base.as_deref(), Some("https://api.example/v1"));
    assert_eq!(provider.provider_type, OPENAI_CHAT_PROVIDER_TYPE);
    assert_eq!(provider.max_context_tokens, Some(1048576));

    let list_response = get(
        router.clone(),
        "/api/config/provider/list?provider_type=chat_completion",
    )
    .await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert!(
        list["data"]
            .as_array()
            .expect("providers")
            .iter()
            .any(|provider| provider["provider_source_id"] == "source-a"
                && provider["key"] == REDACTED_SECRET)
    );

    let check_response = get(
        router.clone(),
        "/api/config/provider/check_one?id=source-a/gpt-4.1-mini",
    )
    .await;
    assert_eq!(check_response.status(), StatusCode::OK);
    let check: serde_json::Value = response_json(check_response).await;
    assert_eq!(check["data"]["status"], "available");

    let source_update_response = post_json(
        router.clone(),
        "/api/config/provider_sources/update",
        json!({
            "original_id": "source-a",
            "config": {
                "id": "source-b",
                "type": OPENAI_CHAT_PROVIDER_TYPE,
                "provider": "openai",
                "api_base": "https://api.changed/v1",
                "key": REDACTED_SECRET
            }
        }),
    )
    .await;
    assert_eq!(source_update_response.status(), StatusCode::OK);
    let saved = RuntimeConfig::from_json_file(&path).expect("config should persist source update");
    let provider = saved
        .chat_providers
        .iter()
        .find(|provider| provider.id == "source-a/gpt-4.1-mini")
        .expect("source model provider");
    assert_eq!(provider.provider_source_id.as_deref(), Some("source-b"));
    assert_eq!(provider.api_base.as_deref(), Some("https://api.changed/v1"));
    assert_eq!(provider.api_key.as_deref(), Some("sk-source"));
    assert_eq!(
        saved.provider_sources[0].api_key.as_deref(),
        Some("sk-source")
    );

    let embedding_dim_response = post_json(
        router.clone(),
        "/api/config/provider/get_embedding_dim",
        json!({ "provider_config": { "model": "text-embedding-3-small" } }),
    )
    .await;
    assert_eq!(embedding_dim_response.status(), StatusCode::OK);
    let embedding_dim: serde_json::Value = response_json(embedding_dim_response).await;
    assert_eq!(embedding_dim["data"]["embedding_dimensions"], 1536);

    let delete_response = post_json(
        router,
        "/api/config/provider_sources/delete",
        json!({ "id": "source-b" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let saved = RuntimeConfig::from_json_file(&path).expect("config should persist source delete");
    assert!(
        !saved
            .chat_providers
            .iter()
            .any(|provider| provider.provider_source_id.as_deref() == Some("source-b"))
    );

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_provider_models_discovers_models_via_real_provider_source_request() {
    let path = temp_management_config_path("provider-model-discovery");
    let _ = std::fs::remove_file(&path);
    let base_url = serve_once_http_response(
        "200 OK",
        "application/json",
        r#"{"data":[{"id":"gpt-live"},{"id":"gpt-next"}]}"#,
    )
    .await;
    let mut config = RuntimeConfig::default();
    config.provider_sources =
        vec![RuntimeProviderSourceConfig::openai("source-live", base_url).with_api_key("sk-live")];
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&config).expect("runtime config should serialize"),
    )
    .expect("runtime config should write");
    let state = management_state_fixture().with_config_service(RuntimeConfigService::new(&path));
    let router = management_router(state);

    let response = get(
        router,
        "/api/config/provider_sources/models?source_id=source-live",
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response_json(response).await;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["dynamic"], true);
    assert_eq!(payload["data"]["unsupported"], false);
    assert_eq!(payload["data"]["model_discovery"], "supported");
    assert_eq!(payload["data"]["capability"], "chat_completion");
    assert_eq!(
        payload["data"]["models"]
            .as_array()
            .expect("models")
            .iter()
            .map(|value| value.as_str().expect("model").to_string())
            .collect::<Vec<_>>(),
        vec!["gpt-live".to_string(), "gpt-next".to_string()]
    );

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_provider_models_reports_unsupported_provider_metadata() {
    let path = temp_management_config_path("provider-model-unsupported");
    let _ = std::fs::remove_file(&path);
    let mut config = RuntimeConfig::default();
    config.provider_sources = vec![RuntimeProviderSourceConfig {
        id: "source-anthropic".to_string(),
        provider_type: astrbot_provider::ANTHROPIC_CHAT_PROVIDER_TYPE.to_string(),
        enabled: true,
        provider: Some("anthropic".to_string()),
        api_base: Some("http://127.0.0.1:1".to_string()),
        api_key: Some("anthropic-secret".to_string()),
        proxy: None,
        timeout_secs: 5,
        custom_extra_body: serde_json::Value::Null,
    }];
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&config).expect("runtime config should serialize"),
    )
    .expect("runtime config should write");
    let state = management_state_fixture().with_config_service(RuntimeConfigService::new(&path));
    let router = management_router(state);

    let response = get(
        router,
        "/api/config/provider_sources/models?source_id=source-anthropic",
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response_json(response).await;
    assert_eq!(payload["data"]["dynamic"], false);
    assert_eq!(payload["data"]["unsupported"], true);
    assert_eq!(payload["data"]["model_discovery"], "unsupported");
    assert_eq!(payload["data"]["models"][0], "claude-3-5-sonnet-latest");
    assert!(
        !serde_json::to_string(&payload)
            .expect("unsupported json")
            .contains("anthropic-secret")
    );

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_provider_models_redacts_secret_from_runtime_discovery_error() {
    let path = temp_management_config_path("provider-model-redaction");
    let _ = std::fs::remove_file(&path);
    let base_url = serve_once_http_response(
        "401 Unauthorized",
        "application/json",
        r#"{"error":{"message":"token sk-leaked-secret was rejected"}}"#,
    )
    .await;
    let mut config = RuntimeConfig::default();
    config.provider_sources = vec![
        RuntimeProviderSourceConfig::openai("source-redacted", base_url)
            .with_api_key("sk-leaked-secret"),
    ];
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&config).expect("runtime config should serialize"),
    )
    .expect("runtime config should write");
    let state = management_state_fixture().with_config_service(RuntimeConfigService::new(&path));
    let router = management_router(state);

    let response = get(
        router,
        "/api/config/provider_sources/models?source_id=source-redacted",
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response_json(response).await;
    assert_eq!(payload["data"]["dynamic"], false);
    assert_eq!(payload["data"]["error_kind"], "credential");
    let encoded = serde_json::to_string(&payload).expect("redaction json");
    assert!(encoded.contains("<redacted>"));
    assert!(!encoded.contains("sk-leaked-secret"));

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_provider_check_reports_timeout_and_credential_errors_without_secrets() {
    let path = temp_management_config_path("provider-health-errors");
    let _ = std::fs::remove_file(&path);
    let mut config = RuntimeConfig::default();
    config.chat_providers = vec![
        RuntimeChatProviderConfig::openai_compatible(
            "provider-health",
            "https://api.example/v1",
            "gpt-test",
        )
        .with_api_key("sk-secret"),
    ];
    config.default_chat_provider_id = "provider-health".to_string();
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&config).expect("runtime config should serialize"),
    )
    .expect("runtime config should write");

    let timeout_router = management_router(
        management_state_fixture()
            .with_config_service(RuntimeConfigService::new(&path))
            .with_provider_health_check(Arc::new(StaticProviderHealthCheck::timeout())),
    );
    let timeout_response = post_json(
        timeout_router,
        "/api/management/providers/check",
        json!({ "id": "provider-health" }),
    )
    .await;
    assert_eq!(timeout_response.status(), StatusCode::OK);
    let timeout: serde_json::Value = response_json(timeout_response).await;
    assert_eq!(timeout["ok"], false);
    assert_eq!(timeout["error_kind"], "timeout");
    assert!(
        !serde_json::to_string(&timeout)
            .expect("timeout json")
            .contains("sk-secret")
    );

    let credential_router = management_router(
        management_state_fixture()
            .with_config_service(RuntimeConfigService::new(&path))
            .with_provider_health_check(Arc::new(StaticProviderHealthCheck::credential())),
    );
    let credential_response = get(
        credential_router,
        "/api/config/provider/check_one?id=provider-health",
    )
    .await;
    assert_eq!(credential_response.status(), StatusCode::OK);
    let credential: serde_json::Value = response_json(credential_response).await;
    assert_eq!(credential["data"]["status"], "unavailable");
    assert_eq!(credential["data"]["error_kind"], "credential");
    assert!(
        !serde_json::to_string(&credential)
            .expect("credential json")
            .contains("sk-secret")
    );

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_platform_routes_mutate_runtime_config_and_validate_templates() {
    let path = temp_management_config_path("platform-crud");
    let _ = std::fs::remove_file(&path);
    let state = management_state_fixture()
        .with_config_service(RuntimeConfigService::new(&path))
        .with_platform_health_check(Arc::new(StaticPlatformHealthCheck::available()));
    let router = management_router(state);

    let catalog_response = get(router.clone(), "/api/management/platforms/catalog").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert!(
        catalog["templates"]
            .as_array()
            .expect("templates")
            .iter()
            .any(|template| template["platform_type"] == CONSOLE_PLATFORM_TYPE)
    );

    let legacy_config_response = get(router.clone(), "/api/config/get").await;
    assert_eq!(legacy_config_response.status(), StatusCode::OK);
    let legacy_config: serde_json::Value = response_json(legacy_config_response).await;
    assert_eq!(legacy_config["status"], "ok");
    assert!(legacy_config["data"]["config"]["platform"].is_array());
    assert!(
        legacy_config["data"]["metadata"]["platform_group"]["metadata"]["platform"]
            ["config_template"]
            .is_object()
    );

    let legacy_create_response = post_json(
        router.clone(),
        "/api/config/platform/new",
        json!({
            "id": "onebot-main",
            "type": AIOCQHTTP_PLATFORM_TYPE,
            "enable": true,
            "name": "OneBot Main",
            "ws_reverse_host": "0.0.0.0",
            "ws_reverse_port": 6199,
            "ws_reverse_token": "secret-token"
        }),
    )
    .await;
    assert_eq!(legacy_create_response.status(), StatusCode::OK);
    let legacy_created: serde_json::Value = response_json(legacy_create_response).await;
    assert_eq!(legacy_created["status"], "ok");
    assert_eq!(
        legacy_created["data"]["platform"]["ws_reverse_token"],
        REDACTED_SECRET
    );
    assert_eq!(
        legacy_created["data"]["platform"]["secrets"]["ws_reverse_token"],
        REDACTED_SECRET
    );
    let saved = RuntimeConfig::from_json_file(&path).expect("config should persist legacy create");
    let saved_platform = saved
        .platforms
        .iter()
        .find(|platform| platform.id == "onebot-main")
        .expect("legacy platform should be saved");
    assert_eq!(saved_platform.platform_type, AIOCQHTTP_PLATFORM_TYPE);
    assert_eq!(saved_platform.enabled, true);
    assert_eq!(
        saved_platform.options["ws_reverse_port"],
        serde_json::Value::from(6199)
    );
    assert_eq!(
        saved_platform
            .secrets
            .get("ws_reverse_token")
            .map(String::as_str),
        Some("secret-token")
    );

    let legacy_config_response = get(router.clone(), "/api/config/get").await;
    assert_eq!(legacy_config_response.status(), StatusCode::OK);
    let legacy_config: serde_json::Value = response_json(legacy_config_response).await;
    let onebot_config = legacy_config["data"]["config"]["platform"]
        .as_array()
        .expect("legacy platform list")
        .iter()
        .find(|platform| platform["id"] == "onebot-main")
        .expect("legacy platform output");
    assert_eq!(onebot_config["ws_reverse_token"], REDACTED_SECRET);
    assert_eq!(
        onebot_config["secrets"]["ws_reverse_token"],
        REDACTED_SECRET
    );

    let legacy_update_response = post_json(
        router.clone(),
        "/api/config/platform/update",
        json!({
            "id": "onebot-main",
            "config": {
                "id": "onebot-main",
                "type": AIOCQHTTP_PLATFORM_TYPE,
                "enable": false,
                "name": "OneBot Main",
                "ws_reverse_host": "127.0.0.1",
                "ws_reverse_port": 6200,
                "ws_reverse_token": REDACTED_SECRET
            }
        }),
    )
    .await;
    assert_eq!(legacy_update_response.status(), StatusCode::OK);
    let saved = RuntimeConfig::from_json_file(&path).expect("config should persist legacy update");
    let saved_platform = saved
        .platforms
        .iter()
        .find(|platform| platform.id == "onebot-main")
        .expect("legacy platform should still exist");
    assert_eq!(saved_platform.enabled, false);
    assert_eq!(
        saved_platform.options["ws_reverse_host"],
        serde_json::Value::from("127.0.0.1")
    );
    assert_eq!(
        saved_platform.options["ws_reverse_port"],
        serde_json::Value::from(6200)
    );
    assert_eq!(
        saved_platform
            .secrets
            .get("ws_reverse_token")
            .map(String::as_str),
        Some("secret-token")
    );

    let legacy_stats_response = get(router.clone(), "/api/platform/stats").await;
    assert_eq!(legacy_stats_response.status(), StatusCode::OK);
    let legacy_stats: serde_json::Value = response_json(legacy_stats_response).await;
    assert_eq!(legacy_stats["status"], "ok");
    let legacy_stats_platforms = legacy_stats["data"]["platforms"]
        .as_array()
        .expect("legacy stats platforms");
    assert!(
        legacy_stats_platforms
            .iter()
            .any(|platform| platform["id"] == "onebot-main"
                && platform["type"] == AIOCQHTTP_PLATFORM_TYPE
                && platform["status"] == "stopped")
    );
    assert_eq!(
        legacy_stats["data"]["summary"]["total"].as_u64(),
        Some(legacy_stats_platforms.len() as u64)
    );

    let legacy_delete_response = post_json(
        router.clone(),
        "/api/config/platform/delete",
        json!({ "id": "onebot-main" }),
    )
    .await;
    assert_eq!(legacy_delete_response.status(), StatusCode::OK);
    let saved = RuntimeConfig::from_json_file(&path).expect("config should persist legacy delete");
    assert!(
        !saved
            .platforms
            .iter()
            .any(|platform| platform.id == "onebot-main")
    );

    let upsert_response = post_json(
        router.clone(),
        "/api/management/platforms/upsert",
        json!({
            "platform": {
                "id": "console-dashboard",
                "type": CONSOLE_PLATFORM_TYPE,
                "enabled": true,
                "name": "Console Dashboard"
            }
        }),
    )
    .await;
    assert_eq!(upsert_response.status(), StatusCode::OK);
    let upserted: serde_json::Value = response_json(upsert_response).await;
    assert!(
        upserted["catalog"]["platforms"]
            .as_array()
            .expect("platforms")
            .iter()
            .any(|platform| platform["id"] == "console-dashboard")
    );

    let check_response = post_json(
        router.clone(),
        "/api/management/platforms/check",
        json!({ "id": "console-dashboard" }),
    )
    .await;
    assert_eq!(check_response.status(), StatusCode::OK);
    let check: serde_json::Value = response_json(check_response).await;
    assert_eq!(check["ok"], true);
    assert_eq!(check["status"], "available");
    assert!(
        check["message"]
            .as_str()
            .expect("platform check message")
            .contains("adapter")
    );

    let delete_response = post_json(
        router,
        "/api/management/platforms/delete",
        json!({ "id": "console-dashboard" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_response).await;
    assert_eq!(deleted["changed"], true);
    let saved = RuntimeConfig::from_json_file(&path).expect("config should persist delete");
    assert!(
        !saved
            .platforms
            .iter()
            .any(|platform| platform.id == "console-dashboard")
    );

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_platform_legacy_facades_match_source_shapes() {
    let path = temp_management_config_path("platform-legacy");
    let _ = std::fs::remove_file(&path);
    let state = management_state_fixture().with_config_service(RuntimeConfigService::new(&path));
    let router = management_router(state);

    let create_response = post_json(
        router.clone(),
        "/api/config/platform/new",
        json!({
            "id": "onebot-main",
            "type": "onebot",
            "enable": true,
            "name": "OneBot Main",
            "ws_reverse_host": "0.0.0.0",
            "ws_reverse_port": 6199,
            "ws_reverse_token": "secret",
            "unified_webhook": true,
            "webhook_uuid": "uuid-1"
        }),
    )
    .await;
    assert_eq!(create_response.status(), StatusCode::OK);
    let created: serde_json::Value = response_json(create_response).await;
    assert_eq!(created["status"], "ok");
    assert_eq!(created["data"]["platform"]["id"], "onebot-main");
    assert_eq!(created["data"]["platform"]["enable"], true);
    assert_eq!(created["data"]["platform"]["webhook_uuid"], "uuid-1");
    assert_eq!(
        created["data"]["platform"]["ws_reverse_token"],
        REDACTED_SECRET
    );

    let config_response = get(router.clone(), "/api/config/get").await;
    assert_eq!(config_response.status(), StatusCode::OK);
    let config: serde_json::Value = response_json(config_response).await;
    assert_eq!(config["status"], "ok");
    assert!(
        config["data"]["config"]["platform"]
            .as_array()
            .expect("legacy platform list")
            .iter()
            .any(|platform| platform["id"] == "onebot-main" && platform["webhook_uuid"] == "uuid-1")
    );
    assert!(config["data"]["metadata"]["platform_group"].is_object());

    let stats_response = get(router.clone(), "/api/platform/stats").await;
    assert_eq!(stats_response.status(), StatusCode::OK);
    let stats: serde_json::Value = response_json(stats_response).await;
    assert_eq!(stats["status"], "ok");
    assert!(
        stats["data"]["platforms"]
            .as_array()
            .expect("platform stats")
            .iter()
            .any(|platform| platform["id"] == "onebot-main"
                && platform["unified_webhook"] == true
                && platform["webhook_uuid"] == "uuid-1")
    );

    let webhook_response = get(router.clone(), "/api/platform/webhook/uuid-1").await;
    assert_eq!(webhook_response.status(), StatusCode::OK);
    let webhook: serde_json::Value = response_json(webhook_response).await;
    assert_eq!(webhook["status"], "ok");
    assert_eq!(webhook["data"]["platform_id"], "onebot-main");

    let update_response = post_json(
        router.clone(),
        "/api/config/platform/update",
        json!({
            "id": "onebot-main",
            "config": {
                "id": "onebot-main",
                "type": "onebot",
                "enable": false,
                "ws_reverse_host": "0.0.0.0",
                "ws_reverse_port": 6199,
                "ws_reverse_token": REDACTED_SECRET,
                "unified_webhook": true,
                "webhook_uuid": "uuid-1"
            }
        }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);
    let saved =
        RuntimeConfig::from_json_file(&path).expect("legacy platform update should persist");
    let platform = saved
        .platforms
        .iter()
        .find(|platform| platform.id == "onebot-main")
        .expect("platform should be present");
    assert!(!platform.enabled);
    assert_eq!(
        platform.secrets.get("ws_reverse_token").map(String::as_str),
        Some("secret")
    );

    let delete_response = post_json(
        router,
        "/api/config/platform/delete",
        json!({ "id": "onebot-main" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted =
        RuntimeConfig::from_json_file(&path).expect("legacy platform delete should persist");
    assert!(
        !deleted
            .platforms
            .iter()
            .any(|platform| platform.id == "onebot-main")
    );

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_platform_check_reports_success_timeout_credential_and_webhook_state() {
    let path = temp_management_config_path("platform-health");
    let _ = std::fs::remove_file(&path);
    let mut config = RuntimeConfig::default();
    config.platforms = vec![
        RuntimePlatformConfig::new("slack-main", "slack")
            .with_option_string("slack_connection_mode", "webhook")
            .with_option_string("webhook_uuid", "slack-hook")
            .with_secret("bot_token", "xoxb-secret")
            .with_secret("signing_secret", "signing-secret")
            .with_option_u16("slack_webhook_port", 6197),
    ];
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&config).expect("runtime config should serialize"),
    )
    .expect("runtime config should write");

    let success_router = management_router(
        management_state_fixture()
            .with_config_service(RuntimeConfigService::new(&path))
            .with_platform_health_check(Arc::new(StaticPlatformHealthCheck::available())),
    );
    let success_response = post_json(
        success_router,
        "/api/management/platforms/check",
        json!({ "id": "slack-main" }),
    )
    .await;
    assert_eq!(success_response.status(), StatusCode::OK);
    let success: serde_json::Value = response_json(success_response).await;
    assert_eq!(success["ok"], true);
    assert_eq!(success["status"], "available");
    assert_eq!(success["webhook_reachable"], true);

    let timeout_router = management_router(
        management_state_fixture()
            .with_config_service(RuntimeConfigService::new(&path))
            .with_platform_health_check(Arc::new(StaticPlatformHealthCheck::timeout())),
    );
    let timeout_response = post_json(
        timeout_router,
        "/api/management/platforms/check",
        json!({ "id": "slack-main" }),
    )
    .await;
    assert_eq!(timeout_response.status(), StatusCode::OK);
    let timeout: serde_json::Value = response_json(timeout_response).await;
    assert_eq!(timeout["ok"], false);
    assert_eq!(timeout["error_kind"], "timeout");
    assert!(
        !serde_json::to_string(&timeout)
            .expect("timeout json")
            .contains("xoxb-secret")
    );

    let credential_router = management_router(
        management_state_fixture()
            .with_config_service(RuntimeConfigService::new(&path))
            .with_platform_health_check(Arc::new(StaticPlatformHealthCheck::credential())),
    );
    let credential_response = post_json(
        credential_router,
        "/api/management/platforms/check",
        json!({ "id": "slack-main" }),
    )
    .await;
    assert_eq!(credential_response.status(), StatusCode::OK);
    let credential: serde_json::Value = response_json(credential_response).await;
    assert_eq!(credential["ok"], false);
    assert_eq!(credential["error_kind"], "credential");
    assert!(
        !serde_json::to_string(&credential)
            .expect("credential json")
            .contains("xoxb-secret")
    );

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_platform_default_check_classifies_missing_credentials_without_http_error() {
    let path = temp_management_config_path("platform-default-credential");
    let _ = std::fs::remove_file(&path);
    let mut config = RuntimeConfig::default();
    config.platforms = vec![RuntimePlatformConfig::new("telegram-main", "telegram")];
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&config).expect("runtime config should serialize"),
    )
    .expect("runtime config should write");
    let router = management_router(
        management_state_fixture().with_config_service(RuntimeConfigService::new(&path)),
    );

    let response = post_json(
        router,
        "/api/management/platforms/check",
        json!({ "id": "telegram-main" }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response_json(response).await;
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["status"], "unavailable");
    assert_eq!(payload["error_kind"], "credential");

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_dashboard_capabilities_reports_service_closure_levels() {
    let market_entry = PluginMarketEntry::new("market-tools", "Market Tools", "0.3.0")
        .with_package(PluginPackageDescriptor::new(PluginInstallSource::archive(
            "https://example.com/market-tools.zip",
        )));
    let state = management_state_fixture()
        .with_config_service(RuntimeConfigService::new(temp_management_config_path(
            "capabilities",
        )))
        .with_knowledge_base(knowledge_base_management_state_fixture())
        .with_tools(tool_management_state_fixture())
        .with_session_rules({
            let repository = Arc::new(InMemorySessionRuleRepository::new());
            ManagementSessionRuleState::new(repository.clone(), repository)
        })
        .with_chat_projects(ManagementChatProjectState::new(ChatProjectService::new(
            Arc::new(InMemoryChatProjectRepository::new()),
        )))
        .with_plugin_market(PluginMarketManagementState::new(vec![market_entry]))
        .with_skills(skill_management_state_fixture())
        .with_mcp(ManagementMcpState::default())
        .with_backup(backup_management_state("4.9.1"))
        .with_maintenance(ManagementMaintenanceState::new("4.9.1"))
        .with_observability(ManagementObservabilityState::new(
            Arc::new(InMemoryLogBuffer::new(8)),
            Vec::new(),
        ))
        .with_subagents(ManagementSubagentState::new(SubagentConfigSource::new(
            vec![SubagentConfig::new("researcher").with_provider_id("mock-provider")],
        )))
        .with_api_keys(ManagementApiKeyState::new(Arc::new(
            InMemoryApiKeyRepository::new(),
        )));
    let router = management_router(state);

    let response = get(router, "/api/management/dashboard/capabilities").await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: DashboardCapabilitiesResponse = response_json(response).await;
    let status = payload
        .services
        .iter()
        .find(|service| service.id == "status")
        .expect("status capability");
    assert!(status.configured);
    assert_eq!(status.closure_level, DashboardClosureLevel::Runtime);
    let conversation = payload
        .services
        .iter()
        .find(|service| service.id == "conversation")
        .expect("conversation capability");
    assert!(conversation.configured);
    assert_eq!(conversation.closure_level, DashboardClosureLevel::Runtime);
    let market = payload
        .services
        .iter()
        .find(|service| service.id == "plugin_market")
        .expect("plugin market capability");
    assert!(market.configured);
    assert_eq!(market.closure_level, DashboardClosureLevel::InMemory);
    let backup = payload
        .services
        .iter()
        .find(|service| service.id == "backup")
        .expect("backup capability");
    assert!(backup.configured);
    assert_eq!(backup.closure_level, DashboardClosureLevel::InMemory);
    let maintenance = payload
        .services
        .iter()
        .find(|service| service.id == "maintenance")
        .expect("maintenance capability");
    assert!(maintenance.configured);
    assert_eq!(maintenance.closure_level, DashboardClosureLevel::InMemory);
    let subagent = payload
        .services
        .iter()
        .find(|service| service.id == "subagent")
        .expect("subagent capability");
    assert!(subagent.configured);
    assert_eq!(subagent.closure_level, DashboardClosureLevel::Runtime);
    let api_keys = payload
        .services
        .iter()
        .find(|service| service.id == "api_keys")
        .expect("api keys capability");
    assert!(api_keys.configured);
    assert_eq!(api_keys.closure_level, DashboardClosureLevel::InMemory);
    let openapi_chat = payload
        .services
        .iter()
        .find(|service| service.id == "openapi_chat")
        .expect("openapi chat capability");
    assert!(openapi_chat.configured);
    assert_eq!(openapi_chat.closure_level, DashboardClosureLevel::Runtime);
    assert_eq!(openapi_chat.api_base, "/api/openapi/chat");
    let providers = payload
        .services
        .iter()
        .find(|service| service.id == "providers")
        .expect("providers capability");
    assert!(providers.configured);
    assert_eq!(providers.closure_level, DashboardClosureLevel::Runtime);
    let platforms = payload
        .services
        .iter()
        .find(|service| service.id == "platforms")
        .expect("platforms capability");
    assert!(platforms.configured);
    assert_eq!(platforms.closure_level, DashboardClosureLevel::Runtime);
    let commands = payload
        .services
        .iter()
        .find(|service| service.id == "commands")
        .expect("commands capability");
    assert!(commands.configured);
    assert_eq!(commands.closure_level, DashboardClosureLevel::Runtime);
    let mcp = payload
        .services
        .iter()
        .find(|service| service.id == "mcp")
        .expect("mcp capability");
    assert!(mcp.configured);
    assert_eq!(mcp.closure_level, DashboardClosureLevel::InMemory);
    let stats = payload
        .services
        .iter()
        .find(|service| service.id == "stats")
        .expect("stats capability");
    assert!(stats.configured);
    assert_eq!(stats.closure_level, DashboardClosureLevel::InMemory);
}

#[tokio::test]
async fn management_api_key_routes_issue_list_revoke_and_delete_without_exposing_hash() {
    let repository = Arc::new(InMemoryApiKeyRepository::new());
    let state =
        management_state_fixture().with_api_keys(ManagementApiKeyState::new(repository.clone()));
    let router = management_router(state);

    let issue_response = post_json(
        router.clone(),
        "/api/management/api-keys/issue",
        json!({
            "key_id": "key-dashboard",
            "name": "Dashboard automation",
            "secret": "ak_dashboard_secret",
            "scopes": ["management.read", "openapi.chat", "management.read"],
            "created_by": "admin"
        }),
    )
    .await;

    assert_eq!(issue_response.status(), StatusCode::OK);
    let issued: serde_json::Value = response_json(issue_response).await;
    assert_eq!(issued["secret"], "ak_dashboard_secret");
    assert_eq!(issued["issued"]["key_prefix"], "ak_dashboard");
    assert!(issued["issued"]["key_hash"].is_null());
    assert_eq!(issued["api_keys"][0]["scopes"][0], "management.read");
    assert_eq!(issued["api_keys"][0]["scopes"][1], "chat");
    assert!(issued["api_keys"][0]["last_used_at"].is_null());
    assert_eq!(issued["api_keys"][0]["is_expired"], false);

    let stored = repository
        .list_api_keys()
        .await
        .expect("api keys should list")
        .pop()
        .expect("api key should store");
    assert_eq!(stored.key_hash, hash_api_key("ak_dashboard_secret"));
    assert_ne!(stored.key_hash, legacy_sha1_hash("ak_dashboard_secret"));
    assert_eq!(stored.key_prefix, "ak_dashboard");

    let catalog_response = get(router.clone(), "/api/management/api-keys").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["api_keys"][0]["key_id"], "key-dashboard");
    assert_eq!(catalog["api_keys"][0]["active"], true);
    assert!(catalog["api_keys"][0]["key_hash"].is_null());

    let revoke_response = post_json(
        router.clone(),
        "/api/management/api-keys/revoke",
        json!({ "key_id": "key-dashboard", "revoked_at": "2026-05-17T00:00:00Z" }),
    )
    .await;

    assert_eq!(revoke_response.status(), StatusCode::OK);
    let revoked: serde_json::Value = response_json(revoke_response).await;
    assert_eq!(revoked["revoked"], true);
    assert_eq!(revoked["api_keys"][0]["active"], false);
    assert_eq!(revoked["api_keys"][0]["revoked_at"], "2026-05-17T00:00:00Z");

    let delete_response = post_json(
        router,
        "/api/management/api-keys/delete",
        json!({ "key_id": "key-dashboard" }),
    )
    .await;

    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_response).await;
    assert_eq!(deleted["deleted"], true);
    assert_eq!(deleted["api_keys"].as_array().expect("api keys").len(), 0);
}

#[tokio::test]
async fn management_api_key_issue_defaults_match_source_dashboard_contract() {
    let repository = Arc::new(InMemoryApiKeyRepository::new());
    let state =
        management_state_fixture().with_api_keys(ManagementApiKeyState::new(repository.clone()));
    let router = management_router(state);

    let issue_response = post_json(
        router.clone(),
        "/api/management/api-keys/issue",
        json!({ "expires_in_days": 1 }),
    )
    .await;

    assert_eq!(issue_response.status(), StatusCode::OK);
    let issued: serde_json::Value = response_json(issue_response).await;
    let secret = issued["secret"].as_str().expect("secret");
    assert!(secret.starts_with("abk_"));
    assert_eq!(
        issued["issued"]["key_prefix"],
        secret.chars().take(12).collect::<String>()
    );
    assert_eq!(issued["issued"]["created_by"], "dashboard");
    assert_eq!(issued["issued"]["scopes"][0], "chat");
    assert_eq!(issued["issued"]["scopes"][1], "config");
    assert_eq!(issued["issued"]["scopes"][2], "file");
    assert_eq!(issued["issued"]["scopes"][3], "im");
    assert!(issued["issued"]["expires_at"].as_str().is_some());
    assert!(issued["issued"]["key_hash"].is_null());

    let stored = repository
        .list_api_keys()
        .await
        .expect("api keys should list")
        .pop()
        .expect("api key should store");
    assert_eq!(stored.key_hash, hash_api_key(secret));
    assert_ne!(stored.key_hash, legacy_sha1_hash(secret));

    let invalid_response = post_json(
        router,
        "/api/management/api-keys/issue",
        json!({ "scopes": ["chat", "unknown.scope"] }),
    )
    .await;

    assert_eq!(invalid_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn source_compatible_api_key_facades_match_settings_contract() {
    let repository = Arc::new(InMemoryApiKeyRepository::new());
    let state =
        management_state_fixture().with_api_keys(ManagementApiKeyState::new(repository.clone()));
    let router = management_router(state);

    let issue_response = post_json(
        router.clone(),
        "/api/v1/apikeys",
        json!({
            "name": "Dashboard automation",
            "expires_in_days": 7,
            "scopes": ["chat", "file"]
        }),
    )
    .await;
    assert_eq!(issue_response.status(), StatusCode::OK);
    let issued: serde_json::Value = response_json(issue_response).await;
    assert_eq!(issued["status"], "ok");
    assert_eq!(issued["data"]["name"], "Dashboard automation");
    assert!(
        issued["data"]["api_key"]
            .as_str()
            .unwrap()
            .starts_with("abk_")
    );
    let key_id = issued["data"]["key_id"].as_str().unwrap().to_string();

    let list_response = get(router.clone(), "/api/v1/apikeys").await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert_eq!(list["data"][0]["key_id"], key_id);
    assert_eq!(list["data"][0]["is_revoked"], false);
    assert!(list["data"][0]["key_hash"].is_null());

    let revoke_response = post_json(
        router.clone(),
        &format!("/api/v1/apikeys/{key_id}/revoke"),
        json!({}),
    )
    .await;
    assert_eq!(revoke_response.status(), StatusCode::OK);

    let legacy_list_response = get(router.clone(), "/api/apikey/list").await;
    assert_eq!(legacy_list_response.status(), StatusCode::OK);
    let legacy_list: serde_json::Value = response_json(legacy_list_response).await;
    assert_eq!(legacy_list["data"][0]["is_revoked"], true);

    let delete_response = delete(router, &format!("/api/v1/apikeys/{key_id}")).await;
    assert_eq!(delete_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn management_subagent_routes_manage_config_and_handoff_preview() {
    let state = management_state_fixture().with_subagents(ManagementSubagentState::new(
        SubagentConfigSource::new(vec![
            SubagentConfig::new("researcher")
                .with_system_prompt("research carefully")
                .with_tools(["search", "summarize"]),
        ]),
    ));
    let router = management_router(state);

    let catalog_response = get(router.clone(), "/api/management/subagents").await;

    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["agents"][0]["name"], "researcher");
    assert_eq!(
        catalog["handoffs"][0]["tool_name"],
        "transfer_to_researcher"
    );
    assert_eq!(catalog["handoffs"][0]["tools"][0], "search");

    let apply_response = post_json(
        router.clone(),
        "/api/management/subagents/apply",
        json!({
            "agents": [{
                "name": " analyst ",
                "enabled": true,
                "system_prompt": " analyze data ",
                "provider_id": " fast ",
                "tools": [" search ", "search", ""]
            }]
        }),
    )
    .await;

    assert_eq!(apply_response.status(), StatusCode::OK);
    let applied: serde_json::Value = response_json(apply_response).await;
    assert_eq!(applied["ok"], true);
    assert_eq!(applied["catalog"]["agents"][0]["name"], "analyst");
    assert_eq!(applied["catalog"]["agents"][0]["tools"][0], "search");
    assert_eq!(
        applied["catalog"]["handoffs"][0]["tool_name"],
        "transfer_to_analyst"
    );

    let catalog_after_apply = get(router, "/api/management/subagents").await;
    let persisted: serde_json::Value = response_json(catalog_after_apply).await;
    assert_eq!(persisted["agents"][0]["provider_id"], "fast");
}

#[tokio::test]
async fn source_subagent_config_facade_persists_config_and_lists_available_tools() {
    let store = SqliteJsonStore::open_in_memory().expect("sqlite store should open");
    let subagents = ManagementSubagentState::sqlite(
        store.clone(),
        ManagementSubagentConfig {
            agents: vec![SubagentConfig::new("researcher")],
            ..ManagementSubagentConfig::default()
        },
    )
    .expect("subagent state should load");
    let state = management_state_fixture()
        .with_subagents(subagents)
        .with_tools(tool_management_state_fixture());
    let router = management_router(state);

    let initial_response = get(router.clone(), "/api/subagent/config").await;
    assert_eq!(initial_response.status(), StatusCode::OK);
    let initial: serde_json::Value = response_json(initial_response).await;
    assert_eq!(initial["status"], "ok");
    assert_eq!(initial["data"]["agents"][0]["name"], "researcher");

    let update_response = post_json(
        router.clone(),
        "/api/subagent/config",
        json!({
            "main_enable": true,
            "remove_main_duplicate_tools": true,
            "router_system_prompt": "route carefully",
            "agents": [{
                "name": "analyst",
                "enabled": true,
                "persona_id": "persona-a",
                "provider_id": "mock-provider",
                "public_description": "Analyze incidents",
                "system_prompt": "Prefer terse incident summaries",
                "tools": ["weather", "weather", "astr_kb_search"]
            }]
        }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated: serde_json::Value = response_json(update_response).await;
    assert_eq!(updated["message"], "保存成功");
    assert_eq!(updated["data"]["main_enable"], true);
    assert_eq!(updated["data"]["agents"][0]["tools"][0], "astr_kb_search");
    assert_eq!(updated["data"]["agents"][0]["tools"][1], "weather");

    let tools_response = get(router, "/api/subagent/available-tools").await;
    assert_eq!(tools_response.status(), StatusCode::OK);
    let tools: serde_json::Value = response_json(tools_response).await;
    let tool_names = tools["data"]
        .as_array()
        .expect("tools should be an array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name").to_string())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"weather".to_string()));
    assert!(tool_names.contains(&"astr_kb_search".to_string()));

    let reloaded_subagents =
        ManagementSubagentState::sqlite(store, ManagementSubagentConfig::default())
            .expect("subagent state should reload");
    let reloaded_router = management_router(
        management_state_fixture()
            .with_subagents(reloaded_subagents)
            .with_tools(tool_management_state_fixture()),
    );
    let persisted_response = get(reloaded_router, "/api/subagent/config").await;
    let persisted: serde_json::Value = response_json(persisted_response).await;
    assert_eq!(persisted["data"]["main_enable"], true);
    assert_eq!(persisted["data"]["agents"][0]["name"], "analyst");
    assert_eq!(persisted["data"]["agents"][0]["persona_id"], "persona-a");
}

#[tokio::test]
async fn management_subagent_execute_route_uses_configured_execution_bridge() {
    let subagents = ManagementSubagentState::new(SubagentConfigSource::new(vec![
        SubagentConfig::new("researcher")
            .with_provider_id("mock-provider")
            .with_tools(["search"]),
    ]))
    .with_execution_bridge(Arc::new(EchoSubagentBridge));
    let state = management_state_fixture().with_subagents(subagents);
    let router = management_router(state);

    let execute_response = post_json(
        router.clone(),
        "/api/management/subagents/execute",
        json!({
            "agent_name": "researcher",
            "input": "find docs",
            "context": { "route": "subagent" },
            "background": false
        }),
    )
    .await;
    assert_eq!(execute_response.status(), StatusCode::OK);
    let executed: serde_json::Value = response_json(execute_response).await;
    assert_eq!(executed["execution"]["agent_name"], "researcher");
    assert_eq!(executed["execution"]["provider_id"], "mock-provider");
    assert_eq!(executed["execution"]["status"], "completed");
    assert_eq!(executed["execution"]["output"], "researcher: find docs");
    assert_eq!(
        executed["catalog"]["executions"][0]["run_id"],
        "subagent-run-1"
    );

    let catalog_response = get(router, "/api/management/subagents").await;
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["executions"][0]["input"], "find docs");
}

#[tokio::test]
async fn management_router_with_auth_requires_bearer_token() {
    let router = management_router_with_auth(
        management_state_fixture(),
        ManagementAuthState::new(DashboardAuthPolicy::new("secret")),
    );

    let unauthorized = get(router.clone(), "/api/management/status").await;
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let wrong = get_with_bearer(router.clone(), "/api/management/status", "wrong").await;
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    let authorized = get_with_bearer(router, "/api/management/status", "secret").await;
    assert_eq!(authorized.status(), StatusCode::OK);
    let payload: ManagementStatusResponse = response_json(authorized).await;
    assert_eq!(payload.providers.chat_provider_count, 1);
}

#[tokio::test]
async fn dashboard_auth_routes_login_edit_and_invalidate_old_tokens() {
    let path = temp_management_config_path("dashboard-auth");
    let _ = std::fs::remove_file(&path);
    let mut config = RuntimeConfig::default();
    config.dashboard_auth.username = "admin".to_string();
    config.dashboard_auth.password = "old-password".to_string();
    config.dashboard_auth.jwt_secret = "jwt-secret".to_string();
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&config).expect("runtime config should serialize"),
    )
    .expect("runtime config should write");

    let auth = ManagementAuthState::from_config_service(RuntimeConfigService::new(&path));
    let router = management_router_with_auth(management_state_fixture(), auth);

    let login_response = post_json(
        router.clone(),
        "/api/auth/login",
        json!({
            "username": "admin",
            "password": "old-password"
        }),
    )
    .await;
    assert_eq!(login_response.status(), StatusCode::OK);
    let login: serde_json::Value = response_json(login_response).await;
    assert_eq!(login["status"], "ok");
    assert_eq!(login["data"]["username"], "admin");
    assert_eq!(login["data"]["change_pwd_hint"], false);
    let old_token = login["data"]["token"]
        .as_str()
        .expect("token should be present")
        .to_string();

    let authorized = get_with_bearer(router.clone(), "/api/management/status", &old_token).await;
    assert_eq!(authorized.status(), StatusCode::OK);

    let bad_edit = post_json_with_bearer(
        router.clone(),
        "/api/auth/account/edit",
        json!({
            "password": "wrong-password",
            "new_username": "operator"
        }),
        &old_token,
    )
    .await;
    assert_eq!(bad_edit.status(), StatusCode::BAD_REQUEST);

    let edit_response = post_json_with_bearer(
        router.clone(),
        "/api/auth/account/edit",
        json!({
            "password": "old-password",
            "new_username": "operator",
            "new_password": "new-password",
            "confirm_password": "new-password"
        }),
        &old_token,
    )
    .await;
    assert_eq!(edit_response.status(), StatusCode::OK);
    let edited: serde_json::Value = response_json(edit_response).await;
    assert_eq!(edited["status"], "ok");
    assert_eq!(edited["message"], "修改成功");

    let old_token_response =
        get_with_bearer(router.clone(), "/api/management/status", &old_token).await;
    assert_eq!(old_token_response.status(), StatusCode::UNAUTHORIZED);

    let old_login_response = post_json(
        router.clone(),
        "/api/auth/login",
        json!({
            "username": "admin",
            "password": "old-password"
        }),
    )
    .await;
    assert_eq!(old_login_response.status(), StatusCode::UNAUTHORIZED);

    let new_login_response = post_json(
        router.clone(),
        "/api/auth/login",
        json!({
            "username": "operator",
            "password": "new-password"
        }),
    )
    .await;
    assert_eq!(new_login_response.status(), StatusCode::OK);
    let new_login: serde_json::Value = response_json(new_login_response).await;
    let new_token = new_login["data"]["token"]
        .as_str()
        .expect("new token should be present");
    let new_authorized = get_with_bearer(router, "/api/management/status", new_token).await;
    assert_eq!(new_authorized.status(), StatusCode::OK);

    let persisted = RuntimeConfig::from_json_file(&path).expect("saved config should load");
    assert_eq!(persisted.dashboard_auth.username, "operator");
    assert_eq!(persisted.dashboard_auth.password, "new-password");
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn dashboard_auth_routes_reject_expired_runtime_tokens() {
    let path = temp_management_config_path("dashboard-auth-expired");
    let _ = std::fs::remove_file(&path);
    let mut config = RuntimeConfig::default();
    config.dashboard_auth.username = "admin".to_string();
    config.dashboard_auth.password = "password".to_string();
    config.dashboard_auth.jwt_secret = "jwt-secret".to_string();
    config.dashboard_auth.token_ttl_seconds = 0;
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&config).expect("runtime config should serialize"),
    )
    .expect("runtime config should write");

    let auth = ManagementAuthState::from_config_service(RuntimeConfigService::new(&path));
    let router = management_router_with_auth(management_state_fixture(), auth);

    let login_response = post_json(
        router.clone(),
        "/api/auth/login",
        json!({
            "username": "admin",
            "password": "password"
        }),
    )
    .await;
    assert_eq!(login_response.status(), StatusCode::OK);
    let login: serde_json::Value = response_json(login_response).await;
    let token = login["data"]["token"].as_str().expect("token should exist");

    let expired = get_with_bearer(router, "/api/management/status", token).await;
    assert_eq!(expired.status(), StatusCode::UNAUTHORIZED);
    let error: serde_json::Value = response_json(expired).await;
    assert!(error["error"].as_str().expect("error").contains("expired"));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn dashboard_auth_login_rate_limits_and_warns_on_default_credentials() {
    let path = temp_management_config_path("dashboard-auth-security");
    let _ = std::fs::remove_file(&path);
    let config = RuntimeConfig::default();
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&config).expect("runtime config should serialize"),
    )
    .expect("runtime config should write");

    let policy = DashboardAuthPolicy::from_config_service(RuntimeConfigService::new(&path))
        .with_login_rate_limit(1, 60);
    let router =
        management_router_with_auth(management_state_fixture(), ManagementAuthState::new(policy));

    let login_response = post_json(
        router.clone(),
        "/api/auth/login",
        json!({
            "username": "astrbot",
            "password": "77b90590a8945a7d36c963981a307dc9"
        }),
    )
    .await;
    assert_eq!(login_response.status(), StatusCode::OK);
    let login: serde_json::Value = response_json(login_response).await;
    assert_eq!(login["data"]["change_pwd_hint"], true);
    assert!(
        login["data"]["security_warnings"]
            .as_array()
            .expect("security warnings")
            .iter()
            .any(|warning| warning
                .as_str()
                .unwrap_or_default()
                .contains("Default dashboard"))
    );

    let first_bad = post_json(
        router.clone(),
        "/api/auth/login",
        json!({
            "username": "astrbot",
            "password": "wrong"
        }),
    )
    .await;
    assert_eq!(first_bad.status(), StatusCode::UNAUTHORIZED);

    let rate_limited = post_json(
        router,
        "/api/auth/login",
        json!({
            "username": "astrbot",
            "password": "wrong"
        }),
    )
    .await;
    assert_eq!(rate_limited.status(), StatusCode::TOO_MANY_REQUESTS);

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_auth_api_key_scope_blocks_high_risk_write() {
    let repository = Arc::new(InMemoryApiKeyRepository::new());
    repository
        .store_api_key(ApiKeyRecord::new(
            "key-chat",
            "Chat only",
            hash_api_key("ak_chat_only"),
            "ak_chat_only",
            ["chat"],
            "test",
        ))
        .await
        .expect("api key should store");
    let state =
        management_state_fixture().with_api_keys(ManagementApiKeyState::new(repository.clone()));
    let router = management_router_with_auth(
        state,
        ManagementAuthState::new(DashboardAuthPolicy::new("dashboard-secret")),
    );

    let response = post_json_with_headers(
        router,
        "/api/management/config/apply",
        json!({ "config": RuntimeConfig::default() }),
        &[("x-api-key", "ak_chat_only")],
    )
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let error: serde_json::Value = response_json(response).await;
    assert!(
        error["error"]
            .as_str()
            .expect("error message")
            .contains("management scope")
    );
}

#[tokio::test]
async fn management_auth_api_key_management_write_allows_high_risk_write() {
    let repository = Arc::new(InMemoryApiKeyRepository::new());
    repository
        .store_api_key(ApiKeyRecord::new(
            "key-admin",
            "Management writer",
            hash_api_key("ak_management_write"),
            "ak_management",
            ["management.write"],
            "test",
        ))
        .await
        .expect("api key should store");
    let state =
        management_state_fixture().with_api_keys(ManagementApiKeyState::new(repository.clone()));
    let router = management_router_with_auth(
        state,
        ManagementAuthState::new(DashboardAuthPolicy::new("dashboard-secret")),
    );

    let response = post_json_with_headers(
        router,
        "/api/management/api-keys/issue",
        json!({
            "key_id": "issued-by-api-key",
            "name": "Issued by API key",
            "secret": "ak_new_management_key",
            "scopes": ["management.read"],
            "created_by": "api-key"
        }),
        &[("x-api-key", "ak_management_write")],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        repository
            .list_api_keys()
            .await
            .expect("api keys should list")
            .iter()
            .any(|key| key.key_id == "issued-by-api-key")
    );
}

#[tokio::test]
async fn management_audit_logs_high_risk_actions_without_sensitive_fields() {
    let audit_path = temp_management_file_path("management-audit.jsonl");
    let _ = std::fs::remove_file(&audit_path);
    let repository = Arc::new(InMemoryApiKeyRepository::new());
    let state =
        management_state_fixture().with_api_keys(ManagementApiKeyState::new(repository.clone()));
    let auth = ManagementAuthState::new(DashboardAuthPolicy::new("secret"))
        .with_audit_log_file(&audit_path);
    let router = management_router_with_auth(state, auth);

    let response = post_json_with_headers(
        router,
        "/api/management/api-keys/issue",
        json!({
            "key_id": "audit-key",
            "name": "Audit",
            "secret": "ak_should_not_be_logged",
            "scopes": ["management.write"],
            "created_by": "admin"
        }),
        &[
            ("authorization", "Bearer secret"),
            ("x-request-id", "req-audit-1"),
        ],
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let content = std::fs::read_to_string(&audit_path).expect("audit log should persist");
    assert!(content.contains("req-audit-1"));
    assert!(content.contains("\"actor_id\":\"dashboard\""));
    assert!(content.contains("\"status\":200"));
    assert!(!content.contains("ak_should_not_be_logged"));
    assert!(!content.contains("Bearer secret"));
    let entry: serde_json::Value =
        serde_json::from_str(content.lines().next().expect("audit line")).expect("audit JSON");
    assert_eq!(entry["action"], "config_or_access");
    assert_eq!(entry["result"], "success");

    let _ = std::fs::remove_file(audit_path);
}

#[tokio::test]
async fn management_csrf_rejects_cross_origin_mutations() {
    let router = management_router_with_auth(
        management_state_fixture(),
        ManagementAuthState::new(DashboardAuthPolicy::new("secret")),
    );

    let response = post_json_with_headers(
        router,
        "/api/management/config/apply",
        json!({ "config": RuntimeConfig::default() }),
        &[
            ("authorization", "Bearer secret"),
            ("origin", "https://evil.example"),
            ("host", "localhost:6185"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let error: serde_json::Value = response_json(response).await;
    assert!(
        error["error"]
            .as_str()
            .expect("csrf message")
            .contains("cross-origin")
    );
}

#[tokio::test]
async fn management_config_routes_delegate_to_runtime_config_service() {
    let path = temp_management_config_path("apply");
    let _ = std::fs::remove_file(&path);
    let apply_executor = Arc::new(RecordingConfigApplyExecutor::default());
    let state = management_state_fixture()
        .with_config_service(RuntimeConfigService::new(&path))
        .with_config_apply(ManagementConfigApplyState::new(apply_executor.clone()));
    let router = management_router(state);

    let schema_response = get(router.clone(), "/api/management/config/schema").await;
    assert_eq!(schema_response.status(), StatusCode::OK);
    let schema: serde_json::Value = response_json(schema_response).await;
    assert_eq!(schema["schema"]["version"], 1);
    assert!(
        schema["schema"]["fields"]
            .as_array()
            .expect("schema fields should be an array")
            .iter()
            .any(|field| field["path"] == "webchat_server.port")
    );
    assert!(
        schema["ui_metadata"]["groups"]
            .as_array()
            .expect("ui metadata groups should be an array")
            .iter()
            .any(|group| group["id"] == "webchat"
                && group["fields"]
                    .as_array()
                    .expect("ui metadata fields should be an array")
                    .iter()
                    .any(|field| field["path"] == "webchat_server.port"
                        && field["control"] == "number"))
    );

    let current_response = get(router.clone(), "/api/management/config/current").await;
    assert_eq!(current_response.status(), StatusCode::OK);
    let current: serde_json::Value = response_json(current_response).await;
    assert_eq!(current["config"]["webchat_server"]["port"], 6185);

    let mut config = RuntimeConfig::default();
    config.webchat_server.port = 7001;
    let preview_response = post_json(
        router.clone(),
        "/api/management/config/preview",
        json!({ "config": config }),
    )
    .await;
    assert_eq!(preview_response.status(), StatusCode::OK);
    let preview: serde_json::Value = response_json(preview_response).await;
    assert_eq!(preview["plan"]["reload_action"], "restart_runtime");
    assert_eq!(preview["plan"]["changed_fields"], json!(["webchat_server"]));

    let apply_response = post_json(
        router,
        "/api/management/config/apply",
        json!({ "config": preview["config"].clone() }),
    )
    .await;
    assert_eq!(apply_response.status(), StatusCode::OK);
    let applied: serde_json::Value = response_json(apply_response).await;
    assert_eq!(applied["execution"]["accepted"], true);
    assert_eq!(applied["execution"]["action"], "restart_runtime");
    assert_eq!(
        apply_executor.actions(),
        vec![RuntimeConfigReloadAction::RestartRuntime]
    );
    let saved = RuntimeConfig::from_json_file(&path).expect("config should be saved by service");
    assert_eq!(saved.webchat_server.port, 7001);

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_config_apply_rejects_invalid_runtime_config() {
    let path = temp_management_config_path("invalid");
    let _ = std::fs::remove_file(&path);
    let state = management_state_fixture().with_config_service(RuntimeConfigService::new(&path));
    let router = management_router(state);

    let response = post_json(
        router,
        "/api/management/config/apply",
        json!({ "config": { "event_queue_capacity": "large" } }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_config_route_routes_manage_umop_patterns() {
    let state = management_state_fixture().with_config_routes(ManagementConfigRouteState::new(
        UmopConfigRouter::new(vec![UmopConfigRoute::new("onebot::", "onebot-default")])
            .expect("seed route should parse"),
    ));
    let router = management_router(state);

    let catalog_response = get(router.clone(), "/api/management/config/routes").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["routes"][0]["pattern"], "onebot::");

    let upsert_response = post_json(
        router.clone(),
        "/api/management/config/routes/upsert",
        json!({ "pattern": "webchat:group:room-*", "config_id": "room-config" }),
    )
    .await;
    assert_eq!(upsert_response.status(), StatusCode::OK);
    let upserted: serde_json::Value = response_json(upsert_response).await;
    assert_eq!(upserted["changed"], true);

    let resolve_response = post_json(
        router.clone(),
        "/api/management/config/routes/resolve",
        json!({ "umo": "webchat:group:room-alpha" }),
    )
    .await;
    assert_eq!(resolve_response.status(), StatusCode::OK);
    let resolved: serde_json::Value = response_json(resolve_response).await;
    assert_eq!(resolved["config_id"], "room-config");

    let delete_response = post_json(
        router,
        "/api/management/config/routes/delete",
        json!({ "pattern": "webchat:group:room-*" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_response).await;
    assert_eq!(deleted["changed"], true);
}

#[tokio::test]
async fn management_config_routes_persist_abconfs_and_umop_routes() {
    let path = temp_management_config_path("abconf");
    let _ = std::fs::remove_file(&path);
    let service = RuntimeConfigService::new(&path);
    let state = management_state_fixture()
        .with_config_service(service.clone())
        .with_config_routes(
            ManagementConfigRouteState::from_config_service(service.clone())
                .expect("config routes should load"),
        );
    let router = management_router(state);

    let create_response = post_json(
        router.clone(),
        "/api/management/config/abconfs/create",
        json!({
            "name": "Ops",
            "config": RuntimeConfig::default()
        }),
    )
    .await;
    assert_eq!(create_response.status(), StatusCode::OK);
    let created: serde_json::Value = response_json(create_response).await;
    let conf_id = created["conf_id"].as_str().expect("conf id").to_string();
    assert_eq!(created["abconf"]["name"], "Ops");

    let update_response = post_json(
        router.clone(),
        "/api/management/config/abconfs/update",
        json!({ "id": conf_id, "name": "Ops Updated" }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated: serde_json::Value = response_json(update_response).await;
    assert_eq!(updated["abconf"]["name"], "Ops Updated");

    let route_response = post_json(
        router.clone(),
        "/api/management/config/routes/upsert",
        json!({ "pattern": "webchat:group:ops-*", "config_id": conf_id }),
    )
    .await;
    assert_eq!(route_response.status(), StatusCode::OK);

    let reloaded_service = RuntimeConfigService::new(&path);
    let reloaded_state = management_state_fixture()
        .with_config_service(reloaded_service.clone())
        .with_config_routes(
            ManagementConfigRouteState::from_config_service(reloaded_service)
                .expect("persisted config routes should load"),
        );
    let reloaded_router = management_router(reloaded_state);
    let list_response = get(reloaded_router.clone(), "/api/management/config/abconfs").await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert!(
        list["info_list"]
            .as_array()
            .expect("abconf list")
            .iter()
            .any(|item| item["id"] == conf_id && item["name"] == "Ops Updated")
    );

    let resolve_response = post_json(
        reloaded_router.clone(),
        "/api/management/config/routes/resolve",
        json!({ "umo": "webchat:group:ops-room" }),
    )
    .await;
    assert_eq!(resolve_response.status(), StatusCode::OK);
    let resolved: serde_json::Value = response_json(resolve_response).await;
    assert_eq!(resolved["config_id"], conf_id);

    let delete_response = post_json(
        reloaded_router,
        "/api/management/config/abconfs/delete",
        json!({ "id": conf_id }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_response).await;
    assert_eq!(deleted["deleted"], true);

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn management_config_legacy_facades_and_t2i_templates_match_source_shapes() {
    let root = temp_management_dir_path("config-legacy-t2i");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("legacy t2i root should be created");
    let path = root.join("runtime.json");
    let mut config = RuntimeConfig::default();
    config.paths = astrbot_runtime::RuntimePathConfig::default().with_data_dir(root.join("data"));
    fs::write(
        &path,
        serde_json::to_vec_pretty(&config).expect("runtime config should serialize"),
    )
    .expect("runtime config should write");

    let service = RuntimeConfigService::new(&path);
    let state = management_state_fixture()
        .with_config_service(service.clone())
        .with_config_routes(
            ManagementConfigRouteState::from_config_service(service)
                .expect("config route state should load"),
        );
    let router = management_router(state);

    let abconfs_response = get(router.clone(), "/api/config/abconfs").await;
    assert_eq!(abconfs_response.status(), StatusCode::OK);
    let abconfs: serde_json::Value = response_json(abconfs_response).await;
    assert_eq!(abconfs["status"], "ok");
    assert!(
        abconfs["data"]["info_list"]
            .as_array()
            .expect("legacy info list")
            .iter()
            .any(|item| item["id"] == "default")
    );

    let created_response = post_json(
        router.clone(),
        "/api/config/abconf/new",
        json!({ "name": "Ops", "config": RuntimeConfig::default() }),
    )
    .await;
    assert_eq!(created_response.status(), StatusCode::OK);
    let created: serde_json::Value = response_json(created_response).await;
    let conf_id = created["data"]["conf_id"]
        .as_str()
        .expect("legacy conf id")
        .to_string();

    let fetched_response = get(router.clone(), &format!("/api/config/abconf?id={conf_id}")).await;
    assert_eq!(fetched_response.status(), StatusCode::OK);
    let fetched: serde_json::Value = response_json(fetched_response).await;
    assert_eq!(fetched["status"], "ok");
    assert!(fetched["data"]["metadata"].is_object());
    assert_eq!(
        fetched["data"]["config"]["webchat_server"]["port"],
        RuntimeConfig::default().webchat_server.port
    );

    let route_update_response = post_json(
        router.clone(),
        "/api/config/umo_abconf_route/update",
        json!({ "umo": "webchat:group:ops-*", "conf_id": conf_id }),
    )
    .await;
    assert_eq!(route_update_response.status(), StatusCode::OK);

    let route_catalog_response = get(router.clone(), "/api/config/umo_abconf_routes").await;
    assert_eq!(route_catalog_response.status(), StatusCode::OK);
    let route_catalog: serde_json::Value = response_json(route_catalog_response).await;
    assert_eq!(
        route_catalog["data"]["routing"]["webchat:group:ops-*"],
        conf_id
    );

    let template_list_response = get(router.clone(), "/api/t2i/templates").await;
    assert_eq!(template_list_response.status(), StatusCode::OK);
    let template_list: serde_json::Value = response_json(template_list_response).await;
    assert_eq!(template_list["status"], "ok");
    assert!(
        template_list["data"]
            .as_array()
            .expect("template list")
            .iter()
            .any(|item| item["name"] == "base" && item["is_default"] == true)
    );

    let create_template_response = post_json(
        router.clone(),
        "/api/t2i/templates/create",
        json!({ "name": "ops_card", "content": "<main>{{ text }}</main>" }),
    )
    .await;
    assert_eq!(create_template_response.status(), StatusCode::CREATED);

    let get_template_response = get(router.clone(), "/api/t2i/templates/ops_card").await;
    assert_eq!(get_template_response.status(), StatusCode::OK);
    let get_template: serde_json::Value = response_json(get_template_response).await;
    assert_eq!(get_template["data"]["content"], "<main>{{ text }}</main>");

    let update_template_response = put_json(
        router.clone(),
        "/api/t2i/templates/ops_card",
        json!({ "content": "<main>{{ version }}</main>" }),
    )
    .await;
    assert_eq!(update_template_response.status(), StatusCode::OK);

    let active_response = post_json(
        router.clone(),
        "/api/t2i/templates/set_active",
        json!({ "name": "ops_card" }),
    )
    .await;
    assert_eq!(active_response.status(), StatusCode::OK);
    let active: serde_json::Value = response_json(active_response).await;
    assert_eq!(active["data"]["active_template"], "ops_card");

    let active_get_response = get(router.clone(), "/api/t2i/templates/active").await;
    let active_get: serde_json::Value = response_json(active_get_response).await;
    assert_eq!(active_get["data"]["active_template"], "ops_card");

    let delete_template_response = delete(router.clone(), "/api/t2i/templates/ops_card").await;
    assert_eq!(delete_template_response.status(), StatusCode::OK);
    let active_after_delete_response = get(router.clone(), "/api/t2i/templates/active").await;
    let active_after_delete: serde_json::Value = response_json(active_after_delete_response).await;
    assert_eq!(active_after_delete["data"]["active_template"], "base");

    let reset_response = post_json(router, "/api/t2i/templates/reset_default", json!({})).await;
    assert_eq!(reset_response.status(), StatusCode::OK);

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn management_plugin_market_routes_return_catalog_and_side_effect_free_plans() {
    let market_entry = PluginMarketEntry::new("market-tools", "Market Tools", "0.3.0")
        .with_package(
            PluginPackageDescriptor::new(PluginInstallSource::archive(
                "https://example.com/market-tools.zip",
            ))
            .with_checksum_md5("abc123"),
        )
        .with_compatibility(PluginCompatibility::compatible(">=0.1.0"));
    let state = management_state_fixture()
        .with_plugin_market(PluginMarketManagementState::new(vec![market_entry]));
    let router = management_router(state);

    let catalog_response = get(router.clone(), "/api/management/plugin-market").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["plugins"][0]["plugin_id"], "market_tools");
    assert_eq!(catalog["plugins"][0]["package"]["checksum_md5"], "abc123");

    let install_response = post_json(
        router.clone(),
        "/api/management/plugin-market/install-plan",
        json!({ "plugin_id": "market_tools" }),
    )
    .await;
    assert_eq!(install_response.status(), StatusCode::OK);
    let install: serde_json::Value = response_json(install_response).await;
    assert_eq!(install["plan"]["action"], "install");
    assert_eq!(install["plan"]["requires_download"], true);
    assert_eq!(install["plan"]["requires_unpack"], true);
    assert_eq!(install["plan"]["requires_loader_reload"], true);

    let update_response = post_json(
        router.clone(),
        "/api/management/plugin-market/update-plan",
        json!({ "plugin_id": "market_tools" }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update: serde_json::Value = response_json(update_response).await;
    assert_eq!(update["plan"]["action"], "update");
    assert_eq!(update["plan"]["package"]["checksum_md5"], "abc123");

    let uninstall_response = post_json(
        router,
        "/api/management/plugin-market/uninstall-plan",
        json!({ "plugin_id": "market_tools", "delete_config": true }),
    )
    .await;
    assert_eq!(uninstall_response.status(), StatusCode::OK);
    let uninstall: serde_json::Value = response_json(uninstall_response).await;
    assert_eq!(uninstall["plan"]["action"], "uninstall");
    assert_eq!(uninstall["plan"]["requires_download"], false);
    assert_eq!(uninstall["plan"]["delete_config"], true);
}

#[tokio::test]
async fn management_plugin_market_execute_routes_track_installed_state() {
    let market_entry = PluginMarketEntry::new("market-tools", "Market Tools", "0.3.0")
        .with_package(
            PluginPackageDescriptor::new(PluginInstallSource::archive(
                "https://example.com/market-tools.zip",
            ))
            .with_checksum_md5("abc123"),
        )
        .with_compatibility(PluginCompatibility::compatible(">=0.1.0"));
    let state = management_state_fixture()
        .with_plugin_market(PluginMarketManagementState::new(vec![market_entry]));
    let router = management_router(state);

    let install_response = post_json(
        router.clone(),
        "/api/management/plugin-market/install",
        json!({ "plugin_id": "market_tools" }),
    )
    .await;
    assert_eq!(install_response.status(), StatusCode::OK);
    let install: serde_json::Value = response_json(install_response).await;
    assert_eq!(install["operation"]["action"], "install");
    assert_eq!(install["operation"]["status"], "completed");
    assert_eq!(install["installed_plugins"][0]["plugin_id"], "market_tools");
    assert_eq!(install["installed_plugins"][0]["version"], "0.3.0");
    assert_eq!(
        install["installed_plugins"][0]["pending_loader_reload"],
        true
    );

    let catalog_response = get(router.clone(), "/api/management/plugin-market").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["plugins"][0]["installed"], true);
    assert_eq!(catalog["plugins"][0]["installed_version"], "0.3.0");
    assert_eq!(
        catalog["operations"][0]["operation_id"],
        "plugin-market-op-1"
    );

    let update_response = post_json(
        router.clone(),
        "/api/management/plugin-market/update",
        json!({ "plugin_id": "market_tools" }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update: serde_json::Value = response_json(update_response).await;
    assert_eq!(update["operation"]["action"], "update");
    assert_eq!(update["operation"]["operation_id"], "plugin-market-op-2");

    let update_all_plan_response = get(
        router.clone(),
        "/api/management/plugin-market/update-all-plan",
    )
    .await;
    assert_eq!(update_all_plan_response.status(), StatusCode::OK);
    let update_all_plan: serde_json::Value = response_json(update_all_plan_response).await;
    assert_eq!(update_all_plan["plans"][0]["plugin_id"], "market_tools");

    let update_all_response = post_json(
        router.clone(),
        "/api/management/plugin-market/update-all",
        json!({}),
    )
    .await;
    assert_eq!(update_all_response.status(), StatusCode::OK);
    let update_all: serde_json::Value = response_json(update_all_response).await;
    assert_eq!(update_all["operations"][0]["action"], "update");
    assert_eq!(
        update_all["operations"][0]["operation_id"],
        "plugin-market-op-3"
    );

    let uninstall_response = post_json(
        router.clone(),
        "/api/management/plugin-market/uninstall",
        json!({ "plugin_id": "market_tools", "delete_config": true }),
    )
    .await;
    assert_eq!(uninstall_response.status(), StatusCode::OK);
    let uninstall: serde_json::Value = response_json(uninstall_response).await;
    assert_eq!(uninstall["operation"]["action"], "uninstall");
    assert_eq!(uninstall["plan"]["delete_config"], true);
    assert_eq!(
        uninstall["installed_plugins"]
            .as_array()
            .expect("installed plugins")
            .len(),
        0
    );

    let missing_update = post_json(
        router,
        "/api/management/plugin-market/update",
        json!({ "plugin_id": "market_tools" }),
    )
    .await;
    assert_eq!(missing_update.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn management_plugin_lifecycle_routes_manage_state_plans_and_config() {
    let state = management_state_fixture().with_plugin_lifecycle(plugin_lifecycle_state_fixture());
    let router = management_router(state);

    let catalog_response = get(router.clone(), "/api/management/plugins/lifecycle").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["plugins"][0]["plugin_id"], "weather");
    assert_eq!(catalog["plugins"][0]["state"], "active");
    assert_eq!(catalog["handlers"]["handler_count"], 1);

    let disable_response = post_json(
        router.clone(),
        "/api/management/plugins/lifecycle/action",
        json!({ "plugin_id": "weather", "action": "disable" }),
    )
    .await;
    assert_eq!(disable_response.status(), StatusCode::OK);
    let disabled: serde_json::Value = response_json(disable_response).await;
    assert_eq!(disabled["event"]["previous"], "active");
    assert_eq!(disabled["event"]["next"], "disabled");
    assert_eq!(disabled["catalog"]["plugins"][0]["state"], "disabled");

    let reload_response = post_json(
        router.clone(),
        "/api/management/plugins/lifecycle/action",
        json!({ "plugin_id": "weather", "action": "reload" }),
    )
    .await;
    assert_eq!(reload_response.status(), StatusCode::OK);
    let reloaded: serde_json::Value = response_json(reload_response).await;
    assert_eq!(reloaded["event"]["next"], "reloading");

    let upload_plan_response = post_json(
        router.clone(),
        "/api/management/plugins/upload-plan",
        json!({
            "entries": ["weather/main.py", "weather/metadata.yaml"],
            "overwrite": true
        }),
    )
    .await;
    assert_eq!(upload_plan_response.status(), StatusCode::OK);
    let upload_plan: serde_json::Value = response_json(upload_plan_response).await;
    assert_eq!(upload_plan["plugin_id"], "weather");
    assert_eq!(upload_plan["requires_unpack"], true);

    let source_plan_response = post_json(
        router.clone(),
        "/api/management/plugins/source-plan",
        json!({
            "plugin_id": "weather",
            "kind": "python_compat",
            "root_dir": "plugins/weather",
            "module_path": "main.py"
        }),
    )
    .await;
    assert_eq!(source_plan_response.status(), StatusCode::OK);
    let source_plan: serde_json::Value = response_json(source_plan_response).await;
    assert_eq!(source_plan["source"]["kind"], "python_compat");
    assert_eq!(source_plan["source"]["module_path"], "main.py");

    let config_response = post_json(
        router.clone(),
        "/api/management/plugins/config",
        json!({
            "plugin_id": "weather",
            "config": { "city": "Shanghai", "enabled": true }
        }),
    )
    .await;
    assert_eq!(config_response.status(), StatusCode::OK);
    let config: serde_json::Value = response_json(config_response).await;
    assert_eq!(
        config["catalog"]["plugins"][0]["config"]["city"],
        "Shanghai"
    );

    let invalid_upload = post_json(
        router,
        "/api/management/plugins/upload-plan",
        json!({ "entries": ["weather/main.py", "other/main.py"] }),
    )
    .await;
    assert_eq!(invalid_upload.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn management_plugin_config_file_routes_manage_json_files_under_plugin_root() {
    let root = temp_management_file_path("plugin-config-root");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("plugin config root should be created");
    fs::write(root.join("config.json"), r#"{ "city": "Beijing" }"#)
        .expect("plugin config fixture should write");

    let state =
        management_state_fixture().with_plugin_lifecycle(plugin_lifecycle_state_with_root(&root));
    let router = management_router(state);

    let catalog_response = get(router.clone(), "/api/management/plugins/lifecycle").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(
        catalog["plugins"][0]["config_files"][0]["filename"],
        "config.json"
    );

    let list_response = post_json(
        router.clone(),
        "/api/management/plugins/config-file/list",
        json!({ "plugin_id": "weather" }),
    )
    .await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert_eq!(list["files"][0]["filename"], "config.json");

    let read_response = post_json(
        router.clone(),
        "/api/management/plugins/config-file/read",
        json!({ "plugin_id": "weather", "filename": "config.json" }),
    )
    .await;
    assert_eq!(read_response.status(), StatusCode::OK);
    let read: serde_json::Value = response_json(read_response).await;
    assert_eq!(read["config"]["city"], "Beijing");

    let write_response = post_json(
        router.clone(),
        "/api/management/plugins/config-file/write",
        json!({
            "plugin_id": "weather",
            "filename": "dashboard.json",
            "config": { "enabled": true }
        }),
    )
    .await;
    assert_eq!(write_response.status(), StatusCode::OK);
    assert!(root.join("dashboard.json").is_file());

    let delete_response = post_json(
        router.clone(),
        "/api/management/plugins/config-file/delete",
        json!({ "plugin_id": "weather", "filename": "dashboard.json" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_response).await;
    assert_eq!(deleted["deleted"], true);

    let unsafe_response = post_json(
        router,
        "/api/management/plugins/config-file/read",
        json!({ "plugin_id": "weather", "filename": "../config.json" }),
    )
    .await;
    assert_eq!(unsafe_response.status(), StatusCode::BAD_REQUEST);

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn management_skill_routes_expose_catalog_cache_and_side_effect_free_plans() {
    let state = management_state_fixture().with_skills(skill_management_state_fixture());
    let router = management_router(state.clone());

    let catalog_response = get(router.clone(), "/api/management/skills").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["sandbox_cache"]["ready"], true);
    assert_eq!(catalog["skills"][0]["name"], "local_writer");
    assert_eq!(catalog["skills"][0]["source_type"], "local_only");
    assert_eq!(catalog["skills"][1]["name"], "preset");
    assert_eq!(catalog["skills"][1]["source_type"], "sandbox_only");

    let install_response = post_json(
        router.clone(),
        "/api/management/skills/install-plan",
        json!({
            "entries": ["writer/SKILL.md", "writer/assets/template.txt"],
            "overwrite": true
        }),
    )
    .await;
    assert_eq!(install_response.status(), StatusCode::OK);
    let install: serde_json::Value = response_json(install_response).await;
    assert_eq!(install["plan"]["skill_name"], "writer");
    assert_eq!(install["plan"]["requires_unpack"], true);

    let catalog_after_plan_response = get(router.clone(), "/api/management/skills").await;
    let catalog_after_plan: serde_json::Value = response_json(catalog_after_plan_response).await;
    assert!(
        !catalog_after_plan["skills"]
            .as_array()
            .expect("skills array")
            .iter()
            .any(|skill| skill["name"] == "writer")
    );

    let install_execute_response = post_json(
        router.clone(),
        "/api/management/skills/install",
        json!({
            "entries": ["writer/SKILL.md", "writer/assets/template.txt"],
            "overwrite": true
        }),
    )
    .await;
    assert_eq!(install_execute_response.status(), StatusCode::OK);
    let install_execute: serde_json::Value = response_json(install_execute_response).await;
    assert_eq!(install_execute["plan"]["skill_name"], "writer");
    assert_eq!(install_execute["skill"]["name"], "writer");
    assert_eq!(install_execute["skill"]["source_type"], "local_only");

    let catalog_after_install_response = get(router.clone(), "/api/management/skills").await;
    let catalog_after_install: serde_json::Value =
        response_json(catalog_after_install_response).await;
    let installed_skill = catalog_after_install["skills"]
        .as_array()
        .expect("skills array")
        .iter()
        .find(|skill| skill["name"] == "writer")
        .expect("installed writer skill");
    assert_eq!(installed_skill["source_type"], "both");
    assert_eq!(catalog_after_install["sandbox_cache"]["ready"], true);
    let runtime_after_install = state
        .skills()
        .expect("skill state")
        .runtime_snapshot()
        .expect("runtime snapshot");
    let prompt_after_install = SkillPromptRenderer::new()
        .with_runtime(SkillPromptRuntime::Sandbox)
        .render_inventory(
            &runtime_after_install
                .prompt_inventory(&SkillActivationPolicy::all_enabled().allow_only(["writer"])),
        )
        .expect("installed writer should be available in runtime prompt inventory");
    assert!(prompt_after_install.contains("**writer**"));
    assert!(prompt_after_install.contains("/workspace/skills/writer/SKILL.md"));

    let delete_response = post_json(
        router.clone(),
        "/api/management/skills/delete-plan",
        json!({ "name": "local_writer" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete: serde_json::Value = response_json(delete_response).await;
    assert_eq!(delete["plan"]["skill_name"], "local_writer");
    assert_eq!(delete["plan"]["remove_local_dir"], true);

    let delete_execute_response = post_json(
        router.clone(),
        "/api/management/skills/delete",
        json!({ "name": "writer" }),
    )
    .await;
    assert_eq!(delete_execute_response.status(), StatusCode::OK);
    let delete_execute: serde_json::Value = response_json(delete_execute_response).await;
    assert_eq!(delete_execute["plan"]["skill_name"], "writer");
    assert_eq!(delete_execute["deleted"], true);

    let catalog_after_delete_response = get(router.clone(), "/api/management/skills").await;
    let catalog_after_delete: serde_json::Value =
        response_json(catalog_after_delete_response).await;
    assert!(
        !catalog_after_delete["skills"]
            .as_array()
            .expect("skills array")
            .iter()
            .any(|skill| skill["name"] == "writer")
    );

    let synced_delete_response = post_json(
        router.clone(),
        "/api/management/skills/delete",
        json!({ "name": "synced" }),
    )
    .await;
    assert_eq!(synced_delete_response.status(), StatusCode::OK);
    let synced_delete: serde_json::Value = response_json(synced_delete_response).await;
    assert!(synced_delete["remaining_skill"].is_null());
    let catalog_after_synced_delete_response = get(router, "/api/management/skills").await;
    let catalog_after_synced_delete: serde_json::Value =
        response_json(catalog_after_synced_delete_response).await;
    assert!(
        !catalog_after_synced_delete["skills"]
            .as_array()
            .expect("skills array")
            .iter()
            .any(|skill| skill["name"] == "synced")
    );
    assert!(
        catalog_after_synced_delete["skills"]
            .as_array()
            .expect("skills array")
            .iter()
            .any(|skill| skill["name"] == "preset" && skill["source_type"] == "sandbox_only")
    );
}

#[tokio::test]
async fn management_skill_routes_reject_sandbox_only_local_mutations() {
    let state = management_state_fixture().with_skills(skill_management_state_fixture());
    let router = management_router(state);

    let activation_response = post_json(
        router.clone(),
        "/api/management/skills/activation",
        json!({ "name": "preset", "active": false }),
    )
    .await;
    assert_eq!(activation_response.status(), StatusCode::FORBIDDEN);

    let delete_plan_response = post_json(
        router,
        "/api/management/skills/delete-plan",
        json!({ "name": "preset" }),
    )
    .await;
    assert_eq!(delete_plan_response.status(), StatusCode::FORBIDDEN);

    let state = management_state_fixture().with_skills(skill_management_state_fixture());
    let router = management_router(state);
    let delete_response = post_json(
        router,
        "/api/management/skills/delete",
        json!({ "name": "preset" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn management_tool_routes_expose_sources_and_reject_internal_toggle() {
    let state = management_state_fixture().with_tools(tool_management_state_fixture());
    let router = management_router(state);

    let catalog_response = get(router.clone(), "/api/management/tools").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["tools"][0]["name"], "astr_kb_search");
    assert_eq!(catalog["tools"][0]["origin"], "internal");
    assert_eq!(catalog["tools"][0]["origin_name"], "AstrBot");
    assert_eq!(catalog["tools"][0]["user_toggle_allowed"], false);
    assert_eq!(catalog["tools"][1]["name"], "weather");
    assert_eq!(catalog["tools"][1]["origin"], "plugin");
    assert_eq!(catalog["tools"][1]["origin_name"], "Weather Plugin");

    let denied = post_json(
        router.clone(),
        "/api/management/tools/toggle",
        json!({ "name": "astr_kb_search", "active": false }),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let toggled = post_json(
        router.clone(),
        "/api/management/tools/toggle",
        json!({ "name": "weather", "active": false }),
    )
    .await;
    assert_eq!(toggled.status(), StatusCode::OK);
    let response: serde_json::Value = response_json(toggled).await;
    assert_eq!(response["active"], false);

    let catalog_response = get(router, "/api/management/tools").await;
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["tools"][1]["active"], false);
}

#[tokio::test]
async fn management_command_routes_mutate_runtime_config_and_report_conflicts() {
    let path = temp_management_config_path("commands");
    let _ = std::fs::remove_file(&path);
    let config = RuntimeConfig {
        command_plugins: vec![
            RuntimeCommandPluginConfig {
                plugin_name: "builtin".to_string(),
                handler_name: "ping".to_string(),
                command: "ping".to_string(),
                response: "pong".to_string(),
                priority: 10,
                enabled: true,
                permission: Default::default(),
            },
            RuntimeCommandPluginConfig {
                plugin_name: "other".to_string(),
                handler_name: "ping".to_string(),
                command: "ping".to_string(),
                response: "other pong".to_string(),
                priority: 0,
                enabled: true,
                permission: Default::default(),
            },
        ],
        ..RuntimeConfig::default()
    };
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&config).expect("config json"),
    )
    .expect("command config fixture should write");
    let state = management_state_fixture().with_config_service(RuntimeConfigService::new(&path));
    let router = management_router(state);

    let catalog_response = get(router.clone(), "/api/management/commands").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["commands"].as_array().expect("commands").len(), 2);
    assert_eq!(catalog["conflicts"][0]["command"], "ping");

    let update_response = post_json(
        router.clone(),
        "/api/management/commands/update",
        json!({
            "plugin_name": "other",
            "handler_name": "ping",
            "command": "other-ping",
            "enabled": false,
            "permission": "admin"
        }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated: serde_json::Value = response_json(update_response).await;
    assert_eq!(updated["changed"], true);
    assert_eq!(updated["command"]["effective_command"], "other-ping");
    assert_eq!(updated["command"]["permission"], "admin");
    assert_eq!(updated["catalog"]["conflicts"].as_array().unwrap().len(), 0);

    let saved = RuntimeConfig::from_json_file(&path).expect("saved command config");
    let command = saved
        .command_plugins
        .iter()
        .find(|command| command.plugin_name == "other")
        .expect("updated command");
    assert_eq!(command.command, "other-ping");
    assert!(!command.enabled);
    assert_eq!(
        serde_json::to_value(command.permission).expect("permission json"),
        json!("admin")
    );

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_mcp_routes_manage_config_and_generate_sync_plan() {
    let state = management_state_fixture().with_mcp(ManagementMcpState::default());
    let router = management_router(state);

    let empty_response = get(router.clone(), "/api/management/mcp/servers").await;
    assert_eq!(empty_response.status(), StatusCode::OK);
    let empty: serde_json::Value = response_json(empty_response).await;
    assert_eq!(empty["active_count"], 0);

    let upsert_response = post_json(
        router.clone(),
        "/api/management/mcp/servers/upsert",
        json!({
            "name": "Docs Server",
            "server": {
                "active": true,
                "transport": "stdio",
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-filesystem"],
                "sessionReadTimeoutSeconds": 45
            }
        }),
    )
    .await;
    assert_eq!(upsert_response.status(), StatusCode::OK);
    let upserted: serde_json::Value = response_json(upsert_response).await;
    assert_eq!(upserted["changed"], true);
    assert_eq!(upserted["catalog"]["servers"][0]["name"], "Docs Server");
    assert_eq!(upserted["catalog"]["servers"][0]["valid"], true);

    let check_response = post_json(
        router.clone(),
        "/api/management/mcp/servers/check",
        json!({ "name": "Docs Server" }),
    )
    .await;
    assert_eq!(check_response.status(), StatusCode::OK);
    let check: serde_json::Value = response_json(check_response).await;
    assert_eq!(check["ok"], true);
    assert!(
        check["message"]
            .as_str()
            .expect("message")
            .contains("not probed")
    );

    let sync_response = post_json(
        router.clone(),
        "/api/management/mcp/servers/sync",
        json!({ "names": ["Docs Server"] }),
    )
    .await;
    assert_eq!(sync_response.status(), StatusCode::OK);
    let sync: serde_json::Value = response_json(sync_response).await;
    assert_eq!(sync["synced_servers"][0], "Docs Server");
    assert!(
        sync["bridge_tools"]
            .as_array()
            .expect("bridge tools")
            .iter()
            .any(|tool| tool == "mcp_docs_server_read_resource")
    );

    let invalid_response = post_json(
        router.clone(),
        "/api/management/mcp/servers/upsert",
        json!({
            "name": "bad",
            "server": {
                "active": true,
                "transport": "stdio"
            }
        }),
    )
    .await;
    assert_eq!(invalid_response.status(), StatusCode::BAD_REQUEST);

    let delete_response = post_json(
        router,
        "/api/management/mcp/servers/delete",
        json!({ "name": "Docs Server" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_response).await;
    assert_eq!(deleted["changed"], true);
    assert_eq!(deleted["catalog"]["active_count"], 0);
}

#[tokio::test]
async fn management_extension_source_compatible_routes_cover_plugins_tools_mcp_commands_and_skills()
{
    let path = temp_management_config_path("extension-source-compatible");
    let _ = std::fs::remove_file(&path);
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
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&config).expect("config json"),
    )
    .expect("command config fixture should write");

    let market_tools = PluginMarketEntry::new("market-tools", "Market Tools", "0.3.0")
        .with_package(PluginPackageDescriptor::new(PluginInstallSource::archive(
            "https://example.com/market-tools.zip",
        )));
    let upload_tools = PluginMarketEntry::new("upload-tools", "Upload Tools", "0.1.0")
        .with_package(PluginPackageDescriptor::new(PluginInstallSource::archive(
            "https://example.com/upload-tools.zip",
        )));
    let state = management_state_fixture()
        .with_config_service(RuntimeConfigService::new(&path))
        .with_plugin_lifecycle(plugin_lifecycle_state_fixture())
        .with_plugin_market(PluginMarketManagementState::new(vec![
            market_tools,
            upload_tools,
        ]))
        .with_tools(tool_management_state_fixture())
        .with_mcp(ManagementMcpState::default())
        .with_skills(skill_management_state_fixture());
    let router = management_router(state);

    let plugins_response = get(router.clone(), "/api/plugin/get").await;
    assert_eq!(plugins_response.status(), StatusCode::OK);
    let plugins: serde_json::Value = response_json(plugins_response).await;
    assert_eq!(plugins["status"], "ok");
    assert_eq!(plugins["data"][0]["name"], "weather");
    assert_eq!(plugins["data"][0]["activated"], true);

    let readme_response = get(router.clone(), "/api/plugin/readme?name=weather").await;
    assert_eq!(readme_response.status(), StatusCode::OK);
    let readme: serde_json::Value = response_json(readme_response).await;
    assert!(
        readme["data"]["content"]
            .as_str()
            .unwrap()
            .contains("weather")
    );

    let changelog_response = get(router.clone(), "/api/plugin/changelog?name=weather").await;
    assert_eq!(changelog_response.status(), StatusCode::OK);

    let sources_response = get(router.clone(), "/api/plugin/source/get").await;
    assert_eq!(sources_response.status(), StatusCode::OK);
    let sources: serde_json::Value = response_json(sources_response).await;
    assert_eq!(sources["data"][0]["root_dir"], "plugins/weather");

    let save_source_response = post_json(
        router.clone(),
        "/api/plugin/source/save",
        json!({ "sources": [{ "kind": "python_compat", "root_dir": "plugins/weather" }] }),
    )
    .await;
    assert_eq!(save_source_response.status(), StatusCode::OK);

    let failed_plugins_response =
        get(router.clone(), "/api/plugin/source/get-failed-plugins").await;
    assert_eq!(failed_plugins_response.status(), StatusCode::OK);
    let failed_plugins: serde_json::Value = response_json(failed_plugins_response).await;
    assert!(
        failed_plugins["data"]
            .as_array()
            .expect("failed plugins")
            .is_empty()
    );

    let off_response = post_json(
        router.clone(),
        "/api/plugin/off",
        json!({ "name": "weather" }),
    )
    .await;
    assert_eq!(off_response.status(), StatusCode::OK);
    let on_response = post_json(
        router.clone(),
        "/api/plugin/on",
        json!({ "name": "weather" }),
    )
    .await;
    assert_eq!(on_response.status(), StatusCode::OK);
    let reload_response = post_json(
        router.clone(),
        "/api/plugin/reload",
        json!({ "name": "weather" }),
    )
    .await;
    assert_eq!(reload_response.status(), StatusCode::OK);
    let uninstall_failed_response = post_json(
        router.clone(),
        "/api/plugin/uninstall-failed",
        json!({ "dir_name": "broken", "delete_config": true }),
    )
    .await;
    assert_eq!(uninstall_failed_response.status(), StatusCode::OK);

    let market_response = get(router.clone(), "/api/plugin/market_list").await;
    assert_eq!(market_response.status(), StatusCode::OK);
    let market: serde_json::Value = response_json(market_response).await;
    assert_eq!(market["data"][0]["plugin_id"], "market_tools");

    let compat_response = post_json(
        router.clone(),
        "/api/plugin/check-compat",
        json!({ "plugin_id": "market_tools", "astrbot_version": "0.1.0" }),
    )
    .await;
    assert_eq!(compat_response.status(), StatusCode::OK);

    let install_response = post_json(
        router.clone(),
        "/api/plugin/install",
        json!({ "plugin_id": "market_tools" }),
    )
    .await;
    assert_eq!(install_response.status(), StatusCode::OK);
    let install_upload_response = post_json(
        router.clone(),
        "/api/plugin/install-upload",
        json!({ "plugin_id": "upload_tools" }),
    )
    .await;
    assert_eq!(install_upload_response.status(), StatusCode::OK);
    let update_response = post_json(
        router.clone(),
        "/api/plugin/update",
        json!({ "plugin_id": "market_tools" }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_all_response = post_json(router.clone(), "/api/plugin/update-all", json!({})).await;
    assert_eq!(update_all_response.status(), StatusCode::OK);
    let uninstall_response = post_json(
        router.clone(),
        "/api/plugin/uninstall",
        json!({ "plugin_id": "market_tools", "delete_config": true }),
    )
    .await;
    assert_eq!(uninstall_response.status(), StatusCode::OK);

    let commands_response = get(router.clone(), "/api/commands").await;
    assert_eq!(commands_response.status(), StatusCode::OK);
    let commands: serde_json::Value = response_json(commands_response).await;
    assert_eq!(
        commands["data"]["items"][0]["handler_full_name"],
        "builtin.ping"
    );
    let conflicts_response = get(router.clone(), "/api/commands/conflicts").await;
    assert_eq!(conflicts_response.status(), StatusCode::OK);
    let toggle_response = post_json(
        router.clone(),
        "/api/commands/toggle",
        json!({ "handler_full_name": "builtin.ping", "enabled": false }),
    )
    .await;
    assert_eq!(toggle_response.status(), StatusCode::OK);
    let rename_response = post_json(
        router.clone(),
        "/api/commands/rename",
        json!({ "handler_full_name": "builtin.ping", "new_name": "hello" }),
    )
    .await;
    assert_eq!(rename_response.status(), StatusCode::OK);
    let permission_response = post_json(
        router.clone(),
        "/api/commands/permission",
        json!({ "handler_full_name": "builtin.ping", "permission": "admin" }),
    )
    .await;
    assert_eq!(permission_response.status(), StatusCode::OK);

    let tools_response = get(router.clone(), "/api/tools/list").await;
    assert_eq!(tools_response.status(), StatusCode::OK);
    let tools: serde_json::Value = response_json(tools_response).await;
    assert_eq!(tools["data"][1]["name"], "weather");
    let tool_toggle_response = post_json(
        router.clone(),
        "/api/tools/toggle-tool",
        json!({ "name": "weather", "active": false }),
    )
    .await;
    assert_eq!(tool_toggle_response.status(), StatusCode::OK);

    let mcp_empty_response = get(router.clone(), "/api/tools/mcp/servers").await;
    assert_eq!(mcp_empty_response.status(), StatusCode::OK);
    let mcp_add_response = post_json(
        router.clone(),
        "/api/tools/mcp/add",
        json!({
            "name": "Docs Server",
            "active": true,
            "transport": "stdio",
            "command": "npx",
            "args": ["-y", "server"]
        }),
    )
    .await;
    assert_eq!(mcp_add_response.status(), StatusCode::OK);
    let mcp_check_response = post_json(
        router.clone(),
        "/api/tools/mcp/test",
        json!({ "name": "Docs Server" }),
    )
    .await;
    assert_eq!(mcp_check_response.status(), StatusCode::OK);
    let mcp_sync_response = post_json(
        router.clone(),
        "/api/tools/mcp/sync-provider",
        json!({ "name": "Docs Server" }),
    )
    .await;
    assert_eq!(mcp_sync_response.status(), StatusCode::OK);
    let mcp_delete_response = post_json(
        router.clone(),
        "/api/tools/mcp/delete",
        json!({ "name": "Docs Server" }),
    )
    .await;
    assert_eq!(mcp_delete_response.status(), StatusCode::OK);

    let skills_response = get(router.clone(), "/api/skills").await;
    assert_eq!(skills_response.status(), StatusCode::OK);
    let skills: serde_json::Value = response_json(skills_response).await;
    assert_eq!(skills["data"]["skills"][0]["name"], "local_writer");
    let skill_upload_response = post_json(
        router.clone(),
        "/api/skills/upload",
        json!({ "entries": ["writer/SKILL.md"], "overwrite": true }),
    )
    .await;
    assert_eq!(skill_upload_response.status(), StatusCode::OK);
    let skill_batch_response = post_json(
        router.clone(),
        "/api/skills/batch-upload",
        json!({ "entries": ["batch_writer/SKILL.md"], "overwrite": true }),
    )
    .await;
    assert_eq!(skill_batch_response.status(), StatusCode::OK);
    let skill_update_response = post_json(
        router.clone(),
        "/api/skills/update",
        json!({ "name": "local_writer", "active": false }),
    )
    .await;
    assert_eq!(skill_update_response.status(), StatusCode::OK);
    let skill_download_response =
        get(router.clone(), "/api/skills/download?name=local_writer").await;
    assert_eq!(skill_download_response.status(), StatusCode::OK);
    let skill_delete_response = post_json(
        router.clone(),
        "/api/skills/delete",
        json!({ "name": "writer" }),
    )
    .await;
    assert_eq!(skill_delete_response.status(), StatusCode::OK);
    let neo_candidates_response = get(router.clone(), "/api/skills/neo/candidates").await;
    assert_eq!(neo_candidates_response.status(), StatusCode::OK);
    let neo_releases_response = get(router.clone(), "/api/skills/neo/releases").await;
    assert_eq!(neo_releases_response.status(), StatusCode::OK);
    let neo_action_response = post_json(
        router,
        "/api/skills/neo/evaluate",
        json!({ "candidate_id": "cand-1" }),
    )
    .await;
    assert_eq!(neo_action_response.status(), StatusCode::OK);

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn management_knowledge_base_routes_delegate_to_typed_services() {
    let state =
        management_state_fixture().with_knowledge_base(knowledge_base_management_state_fixture());
    let router = management_router(state);

    let preflight_response = post_json(
        router.clone(),
        "/api/management/kb/preflight",
        json!({
            "embedding_provider_id": "embedding",
            "expected_embedding_dimension": 2,
            "rerank_provider_id": "rerank"
        }),
    )
    .await;
    assert_eq!(preflight_response.status(), StatusCode::OK);
    let preflight: serde_json::Value = response_json(preflight_response).await;
    assert_eq!(
        preflight["report"]["embedding"]["actual_dimension"],
        json!(2)
    );
    assert_eq!(preflight["report"]["rerank"]["smoke_test_passed"], true);

    let create_response = post_json(
        router.clone(),
        "/api/management/kb/create",
        json!({
            "kb_id": "kb-1",
            "name": "Docs",
            "description": "Project docs",
            "embedding_provider_id": "embedding",
            "rerank_provider_id": "rerank",
            "chunk_size": 256,
            "chunk_overlap": 32
        }),
    )
    .await;
    assert_eq!(create_response.status(), StatusCode::OK);
    let created: serde_json::Value = response_json(create_response).await;
    assert_eq!(created["knowledge_base"]["kb_id"], "kb-1");
    assert_eq!(created["knowledge_base"]["stats"]["doc_count"], 0);

    let catalog_response = get(router.clone(), "/api/management/kb/catalog").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["knowledge_bases"][0]["name"], "Docs");

    let ingest_response = post_json(
        router.clone(),
        "/api/management/kb/ingest",
        json!({
            "kb_id": "kb-1",
            "doc_id": "doc-ingest",
            "name": "Intro",
            "source_kind": "url",
            "source_url": "https://example.invalid/intro",
            "content": "<h1>Dashboard search</h1><p>Knowledge ingest writes chunks.</p>",
            "clean_html": true
        }),
    )
    .await;
    assert_eq!(ingest_response.status(), StatusCode::OK);
    let ingested: serde_json::Value = response_json(ingest_response).await;
    assert_eq!(ingested["document"]["doc_id"], "doc-ingest");
    assert_eq!(ingested["document"]["chunk_count"], 1);
    assert!(
        !ingested["chunks"][0]["content"]
            .as_str()
            .expect("chunk content")
            .contains("<h1>")
    );

    let documents_response = post_json(
        router.clone(),
        "/api/management/kb/document/list",
        json!({ "kb_id": "kb-1" }),
    )
    .await;
    assert_eq!(documents_response.status(), StatusCode::OK);
    let documents: serde_json::Value = response_json(documents_response).await;
    assert_eq!(documents["documents"][0]["doc_id"], "doc-ingest");

    let retrieve_response = post_json(
        router.clone(),
        "/api/management/kb/retrieve",
        json!({
            "query": "Dashboard search",
            "kb_ids": ["kb-1"],
            "top_k": 1
        }),
    )
    .await;
    assert_eq!(retrieve_response.status(), StatusCode::OK);
    let retrieved: serde_json::Value = response_json(retrieve_response).await;
    assert_eq!(retrieved["mode"], "hybrid_vector");
    assert_eq!(retrieved["results"][0]["doc_id"], "doc-ingest");

    let delete_document_response = post_json(
        router.clone(),
        "/api/management/kb/document/delete",
        json!({ "doc_id": "doc-ingest" }),
    )
    .await;
    assert_eq!(delete_document_response.status(), StatusCode::OK);
    let empty_retrieve_response = post_json(
        router.clone(),
        "/api/management/kb/retrieve",
        json!({
            "query": "Dashboard search",
            "kb_ids": ["kb-1"],
            "top_k": 1
        }),
    )
    .await;
    assert_eq!(empty_retrieve_response.status(), StatusCode::OK);
    let empty_retrieved: serde_json::Value = response_json(empty_retrieve_response).await;
    assert!(
        empty_retrieved["results"]
            .as_array()
            .expect("results")
            .is_empty()
    );

    let plan_response = post_json(
        router.clone(),
        "/api/management/kb/upload/plan",
        json!({
            "task_id": "upload-1",
            "kb_id": "kb-1",
            "kind": "upload",
            "file_total": 1
        }),
    )
    .await;
    assert_eq!(plan_response.status(), StatusCode::OK);
    let planned: serde_json::Value = response_json(plan_response).await;
    assert_eq!(planned["task"]["status"], "pending");

    let progress_response = post_json(
        router.clone(),
        "/api/management/kb/upload/progress",
        json!({
            "task_id": "upload-1",
            "file_index": 0,
            "file_total": 1,
            "file_name": "intro.txt",
            "stage": "embedding",
            "current": 1,
            "total": 2
        }),
    )
    .await;
    assert_eq!(progress_response.status(), StatusCode::OK);
    let progress: serde_json::Value = response_json(progress_response).await;
    assert_eq!(progress["task"]["status"], "processing");
    assert_eq!(progress["task"]["progress"]["stage"], "embedding");

    let complete_response = post_json(
        router.clone(),
        "/api/management/kb/upload/complete",
        json!({
            "task_id": "upload-1",
            "document_ids": ["doc-1"],
            "chunk_count": 2
        }),
    )
    .await;
    assert_eq!(complete_response.status(), StatusCode::OK);

    let poll_response = get(router, "/api/management/kb/upload/progress/upload-1").await;
    assert_eq!(poll_response.status(), StatusCode::OK);
    let task: serde_json::Value = response_json(poll_response).await;
    assert_eq!(task["task"]["status"], "completed");
    assert_eq!(task["task"]["result"]["doc_count"], 1);
}

#[tokio::test]
async fn management_knowledge_base_retrieve_route_searches_management_chunks() {
    let state = management_state_fixture()
        .with_knowledge_base(seeded_knowledge_base_management_state_fixture().await);
    let router = management_router(state);

    let retrieve_response = post_json(
        router.clone(),
        "/api/management/kb/retrieve",
        json!({
            "query": "dashboard search",
            "kb_ids": ["kb-1"],
            "top_k": 2
        }),
    )
    .await;

    assert_eq!(retrieve_response.status(), StatusCode::OK);
    let retrieved: serde_json::Value = response_json(retrieve_response).await;
    assert_eq!(retrieved["mode"], "hybrid_vector");
    assert_eq!(retrieved["results"][0]["chunk_id"], "chunk-dashboard");
    assert_eq!(retrieved["results"][0]["doc_name"], "Intro");

    let missing_response = post_json(
        router,
        "/api/management/kb/retrieve",
        json!({
            "query": "dashboard",
            "kb_ids": ["missing"]
        }),
    )
    .await;
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn source_compatible_kb_routes_cover_native_crud_upload_import_and_retrieval() {
    let state =
        management_state_fixture().with_knowledge_base(knowledge_base_management_state_fixture());
    let router = management_router(state);

    let create_response = post_json(
        router.clone(),
        "/api/kb/create",
        json!({
            "kb_id": "kb-source",
            "kb_name": "Source Docs",
            "description": "Legacy source facade",
            "emoji": "📚",
            "embedding_provider_id": "embedding",
            "rerank_provider_id": "rerank",
            "chunk_size": 256,
            "chunk_overlap": 32,
            "top_k_dense": 3,
            "top_k_sparse": 2,
            "top_m_final": 2
        }),
    )
    .await;
    assert_eq!(create_response.status(), StatusCode::OK);
    let created: serde_json::Value = response_json(create_response).await;
    assert_eq!(created["status"], "ok");
    assert_eq!(created["data"]["kb_id"], "kb-source");
    assert_eq!(created["data"]["kb_name"], "Source Docs");
    assert_eq!(created["data"]["top_m_final"], 2);

    let list_response = get(router.clone(), "/api/kb/list?refresh_stats=true").await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert_eq!(list["data"]["items"][0]["kb_id"], "kb-source");

    let get_response = get(router.clone(), "/api/kb/get?kb_id=kb-source").await;
    assert_eq!(get_response.status(), StatusCode::OK);
    let detail: serde_json::Value = response_json(get_response).await;
    assert_eq!(detail["data"]["kb_name"], "Source Docs");

    let update_response = post_json(
        router.clone(),
        "/api/kb/update",
        json!({
            "kb_id": "kb-source",
            "kb_name": "Source Docs Updated",
            "description": "Updated",
            "top_m_final": 4
        }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated: serde_json::Value = response_json(update_response).await;
    assert_eq!(updated["data"]["kb_name"], "Source Docs Updated");
    assert_eq!(updated["data"]["top_m_final"], 4);

    let boundary = "kb-source-boundary";
    let upload_body = multipart_body(
        boundary,
        &[
            ("kb_id", "kb-source"),
            ("chunk_size", "256"),
            ("chunk_overlap", "32"),
        ],
        "file",
        "intro.txt",
        "text/plain",
        b"Dashboard source upload stores searchable knowledge chunks.",
    );
    let upload_response = post_multipart(
        router.clone(),
        "/api/kb/document/upload",
        boundary,
        upload_body,
    )
    .await;
    assert_eq!(upload_response.status(), StatusCode::OK);
    let uploaded: serde_json::Value = response_json(upload_response).await;
    assert_eq!(uploaded["data"]["file_count"], 1);
    let upload_task_id = uploaded["data"]["task_id"]
        .as_str()
        .expect("upload task id");
    let progress_response = get(
        router.clone(),
        &format!("/api/kb/document/upload/progress?task_id={upload_task_id}"),
    )
    .await;
    assert_eq!(progress_response.status(), StatusCode::OK);
    let upload_progress: serde_json::Value = response_json(progress_response).await;
    assert_eq!(upload_progress["data"]["status"], "completed");
    assert_eq!(upload_progress["data"]["result"]["doc_count"], 1);

    let import_response = post_json(
        router.clone(),
        "/api/kb/document/import",
        json!({
            "kb_id": "kb-source",
            "documents": [{
                "doc_id": "doc-imported",
                "file_name": "imported.md",
                "file_type": "markdown",
                "chunks": ["Imported chunks also enter retrieval."]
            }]
        }),
    )
    .await;
    assert_eq!(import_response.status(), StatusCode::OK);
    let imported: serde_json::Value = response_json(import_response).await;
    assert_eq!(imported["data"]["doc_count"], 1);

    let url_response = post_json(
        router.clone(),
        "/api/kb/document/upload/url",
        json!({
            "kb_id": "kb-source",
            "doc_id": "doc-url",
            "url": "https://example.invalid/docs",
            "content": "<h1>URL knowledge</h1><p>retrieval source facade</p>",
            "enable_cleaning": true
        }),
    )
    .await;
    assert_eq!(url_response.status(), StatusCode::OK);
    let url_upload: serde_json::Value = response_json(url_response).await;
    let url_task_id = url_upload["data"]["task_id"].as_str().expect("url task id");
    let url_progress_response = get(
        router.clone(),
        &format!("/api/kb/document/upload/progress?task_id={url_task_id}"),
    )
    .await;
    assert_eq!(url_progress_response.status(), StatusCode::OK);
    let url_progress: serde_json::Value = response_json(url_progress_response).await;
    assert_eq!(url_progress["data"]["status"], "completed");

    let documents_response = get(router.clone(), "/api/kb/document/list?kb_id=kb-source").await;
    assert_eq!(documents_response.status(), StatusCode::OK);
    let documents: serde_json::Value = response_json(documents_response).await;
    assert!(
        documents["data"]["items"]
            .as_array()
            .expect("documents")
            .iter()
            .any(|document| document["doc_id"] == "doc-url")
    );

    let stats_response = get(router.clone(), "/api/kb/stats?kb_id=kb-source").await;
    assert_eq!(stats_response.status(), StatusCode::OK);
    let stats: serde_json::Value = response_json(stats_response).await;
    assert_eq!(stats["data"]["doc_count"], 3);

    let document_response = get(
        router.clone(),
        "/api/kb/document/get?kb_id=kb-source&doc_id=doc-url",
    )
    .await;
    assert_eq!(document_response.status(), StatusCode::OK);
    let document: serde_json::Value = response_json(document_response).await;
    assert_eq!(document["data"]["doc_id"], "doc-url");

    let chunk_response = get(router.clone(), "/api/kb/chunk/list?doc_id=doc-url").await;
    assert_eq!(chunk_response.status(), StatusCode::OK);
    let chunks: serde_json::Value = response_json(chunk_response).await;
    let chunk_id = chunks["data"]["items"][0]["chunk_id"]
        .as_str()
        .expect("chunk id")
        .to_string();
    assert!(
        !chunks["data"]["items"][0]["content"]
            .as_str()
            .expect("chunk content")
            .contains("<h1>")
    );

    let retrieve_response = post_json(
        router.clone(),
        "/api/kb/retrieve",
        json!({
            "query": "retrieval source facade",
            "kb_ids": ["kb-source"],
            "top_k": 2
        }),
    )
    .await;
    assert_eq!(retrieve_response.status(), StatusCode::OK);
    let retrieved: serde_json::Value = response_json(retrieve_response).await;
    assert_eq!(retrieved["data"]["mode"], "hybrid_vector");
    assert!(
        retrieved["data"]["results"]
            .as_array()
            .expect("results")
            .iter()
            .any(|hit| hit["doc_id"] == "doc-url")
    );

    let delete_chunk_response = post_json(
        router.clone(),
        "/api/kb/chunk/delete",
        json!({ "chunk_id": chunk_id }),
    )
    .await;
    assert_eq!(delete_chunk_response.status(), StatusCode::OK);

    let delete_document_response = post_json(
        router.clone(),
        "/api/kb/document/delete",
        json!({ "doc_id": "doc-url" }),
    )
    .await;
    assert_eq!(delete_document_response.status(), StatusCode::OK);

    let delete_kb_response = post_json(
        router.clone(),
        "/api/kb/delete",
        json!({ "kb_id": "kb-source" }),
    )
    .await;
    assert_eq!(delete_kb_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_kb_response).await;
    assert_eq!(deleted["data"]["deleted"], true);
}

#[tokio::test]
async fn management_update_routes_delegate_to_typed_maintenance_state() {
    let migration_check = MaintenanceMigrationCheck {
        runtime_config: RuntimeConfigMigrationDescriptor {
            missing_default_keys: vec!["webchat_server.port".to_string()],
        },
        pending_storage_migrations: vec!["001-main-schema".to_string()],
        legacy_data_migration_needed: true,
    };
    let maintenance_executor = Arc::new(RecordingMaintenanceExecutor::success());
    let state = management_state_fixture().with_maintenance(
        ManagementMaintenanceState::new("v4.0.0")
            .with_latest_version("v4.1.0")
            .with_dashboard_version("v4.0.0")
            .with_release_notes(vec![
                ReleaseMetadata::new("v4.1.0").with_title("Dashboard parity"),
            ])
            .with_release_executor(maintenance_executor.clone())
            .with_package_executor(maintenance_executor.clone())
            .with_migration_executor(maintenance_executor.clone())
            .with_restart_executor(Arc::new(EchoRestartExecutor))
            .with_migration_check(migration_check),
    );
    let router = management_router(state);

    let check_response = get(router.clone(), "/api/management/update/check").await;
    assert_eq!(check_response.status(), StatusCode::OK);
    let check: serde_json::Value = response_json(check_response).await;
    assert_eq!(check["check"]["has_new_version"], true);
    assert_eq!(check["check"]["dashboard_has_new_version"], false);

    let changelog_response = get(router.clone(), "/api/management/update/changelog").await;
    assert_eq!(changelog_response.status(), StatusCode::OK);
    let changelog: serde_json::Value = response_json(changelog_response).await;
    assert_eq!(changelog["releases"][0]["title"], "Dashboard parity");

    let project_response = post_json(
        router.clone(),
        "/api/management/update/project-plan",
        json!({
            "version": "v4.1.0",
            "proxy": "https://proxy.example/",
            "reboot": true
        }),
    )
    .await;
    assert_eq!(project_response.status(), StatusCode::OK);
    let project: serde_json::Value = response_json(project_response).await;
    assert_eq!(project["operation"]["kind"], "project_update");
    assert_eq!(
        project["operation"]["operation_id"],
        "project-update-v4.1.0"
    );
    assert_eq!(project["operation"]["progress"]["status"], "running");

    let operation_response = get(
        router.clone(),
        "/api/management/update/operations/project-update-v4.1.0",
    )
    .await;
    assert_eq!(operation_response.status(), StatusCode::OK);

    let run_response = post_json(
        router.clone(),
        "/api/management/update/operations/run",
        json!({ "operation_id": "project-update-v4.1.0", "confirmed": true }),
    )
    .await;
    assert_eq!(run_response.status(), StatusCode::OK);
    let run: serde_json::Value = response_json(run_response).await;
    assert_eq!(run["operation"]["progress"]["status"], "completed");

    let dashboard_response = post_json(
        router.clone(),
        "/api/management/update/dashboard-plan",
        json!({ "version": "v4.0.0" }),
    )
    .await;
    assert_eq!(dashboard_response.status(), StatusCode::OK);
    let dashboard: serde_json::Value = response_json(dashboard_response).await;
    assert_eq!(dashboard["operation"]["kind"], "dashboard_update");

    let package_response = post_json(
        router.clone(),
        "/api/management/update/package-plan",
        json!({ "package": "requests==2.32.0", "mirror": "https://mirror.example/simple" }),
    )
    .await;
    assert_eq!(package_response.status(), StatusCode::OK);
    let package: serde_json::Value = response_json(package_response).await;
    assert_eq!(package["plan"]["global_runtime_install"], true);
    assert_eq!(
        package["plan"]["plugin_dependency_plan"],
        serde_json::Value::Null
    );

    let package_run_response = post_json(
        router.clone(),
        "/api/management/update/package-run",
        json!({
            "package": "requests==2.32.0",
            "mirror": "https://mirror.example/simple",
            "confirmed": true
        }),
    )
    .await;
    assert_eq!(package_run_response.status(), StatusCode::OK);
    let package_run: serde_json::Value = response_json(package_run_response).await;
    assert_eq!(package_run["operation"]["kind"], "package_install");
    assert_eq!(package_run["operation"]["progress"]["status"], "completed");

    let restart_plan_response = post_json(
        router.clone(),
        "/api/management/update/restart-plan",
        json!({ "reason": "test restart", "delay_secs": 0 }),
    )
    .await;
    assert_eq!(restart_plan_response.status(), StatusCode::OK);
    let restart_plan: serde_json::Value = response_json(restart_plan_response).await;
    assert_eq!(restart_plan["operation"]["kind"], "restart");
    assert_eq!(restart_plan["operation"]["progress"]["status"], "running");

    let restart_unconfirmed_response = post_json(
        router.clone(),
        "/api/management/update/operations/run",
        json!({ "operation_id": "runtime-restart-test-restart" }),
    )
    .await;
    assert_eq!(restart_unconfirmed_response.status(), StatusCode::OK);
    let restart_unconfirmed: serde_json::Value = response_json(restart_unconfirmed_response).await;
    assert_eq!(
        restart_unconfirmed["operation"]["progress"]["status"],
        "failed"
    );
    assert_eq!(
        restart_unconfirmed["operation"]["progress"]["error"],
        "maintenance operation requires explicit confirmation"
    );

    let restart_replan_response = post_json(
        router.clone(),
        "/api/management/update/restart-plan",
        json!({ "reason": "test restart", "delay_secs": 0 }),
    )
    .await;
    assert_eq!(restart_replan_response.status(), StatusCode::OK);
    let restart_confirmed_response = post_json(
        router.clone(),
        "/api/management/update/operations/run",
        json!({ "operation_id": "runtime-restart-test-restart", "confirmed": true }),
    )
    .await;
    assert_eq!(restart_confirmed_response.status(), StatusCode::OK);
    let restart_confirmed: serde_json::Value = response_json(restart_confirmed_response).await;
    assert_eq!(
        restart_confirmed["operation"]["progress"]["status"],
        "completed"
    );
    assert_eq!(
        restart_confirmed["operation"]["progress"]["events"][1]["message"],
        "restart accepted: test restart"
    );

    let restart_run_response = post_json(
        router.clone(),
        "/api/management/update/restart-run",
        json!({ "reason": "test restart", "delay_secs": 0 }),
    )
    .await;
    assert_eq!(restart_run_response.status(), StatusCode::OK);
    let restart_run: serde_json::Value = response_json(restart_run_response).await;
    assert_eq!(restart_run["operation"]["kind"], "restart");
    assert_eq!(restart_run["operation"]["progress"]["status"], "completed");

    let operations_response = get(router.clone(), "/api/management/update/operations").await;
    assert_eq!(operations_response.status(), StatusCode::OK);
    let operations: serde_json::Value = response_json(operations_response).await;
    assert!(
        operations["operations"]
            .as_array()
            .expect("operations")
            .len()
            >= 2
    );

    let migration_check_response =
        get(router.clone(), "/api/management/update/migration-check").await;
    assert_eq!(migration_check_response.status(), StatusCode::OK);
    let migration_check: serde_json::Value = response_json(migration_check_response).await;
    assert_eq!(
        migration_check["check"]["pending_storage_migrations"],
        json!(["001-main-schema"])
    );
    assert_eq!(
        migration_check["check"]["legacy_data_migration_needed"],
        true
    );

    let migration_response = post_json(
        router,
        "/api/management/update/migration-plan",
        json!({
            "confirmed": true,
            "platform_id_map": {
                "aiocqhttp": { "default": "onebot" }
            }
        }),
    )
    .await;
    assert_eq!(migration_response.status(), StatusCode::OK);
    let migration: serde_json::Value = response_json(migration_response).await;
    assert_eq!(migration["operation"]["kind"], "migration");
    assert_eq!(migration["operation"]["progress"]["status"], "completed");
}

#[tokio::test]
async fn source_compatible_stat_and_update_facades_wrap_maintenance_state() {
    let logs = Arc::new(InMemoryLogBuffer::new(8));
    let maintenance_executor = Arc::new(RecordingMaintenanceExecutor::success());
    let state = management_state_fixture()
        .with_maintenance(
            ManagementMaintenanceState::new("v4.0.0")
                .with_latest_version("v4.1.0")
                .with_dashboard_version("v4.0.0")
                .with_release_notes(vec![
                    ReleaseMetadata::new("v4.1.0").with_title("Dashboard parity"),
                ])
                .with_release_executor(maintenance_executor.clone())
                .with_package_executor(maintenance_executor.clone())
                .with_migration_executor(maintenance_executor)
                .with_restart_executor(Arc::new(EchoRestartExecutor))
                .with_migration_check(MaintenanceMigrationCheck {
                    runtime_config: RuntimeConfigMigrationDescriptor {
                        missing_default_keys: vec![],
                    },
                    pending_storage_migrations: vec!["001-main".to_string()],
                    legacy_data_migration_needed: true,
                }),
        )
        .with_observability(
            ManagementObservabilityState::new(logs, Vec::new()).with_metrics(vec![
                MetricEvent::platform_message("2026-05-17T08:00:00Z", "webchat", "webchat", 2),
            ]),
        );
    let router = management_router(state);

    let version_response = get(router.clone(), "/api/stat/version").await;
    assert_eq!(version_response.status(), StatusCode::OK);
    let version: serde_json::Value = response_json(version_response).await;
    assert_eq!(version["status"], "ok");
    assert_eq!(version["data"]["version"], "v4.0.0");
    assert_eq!(version["data"]["need_migration"], true);

    let stats_response = get(router.clone(), "/api/stat/get").await;
    assert_eq!(stats_response.status(), StatusCode::OK);
    let stats: serde_json::Value = response_json(stats_response).await;
    assert_eq!(stats["data"]["message_count"], 2);
    assert!(stats["data"]["running"]["seconds"].is_number());

    let check_response = get(router.clone(), "/api/update/check").await;
    assert_eq!(check_response.status(), StatusCode::OK);
    let check: serde_json::Value = response_json(check_response).await;
    assert_eq!(check["data"]["version"], "v4.0.0");
    assert_eq!(check["data"]["has_new_version"], true);

    let changelog_list_response = get(router.clone(), "/api/stat/changelog/list").await;
    assert_eq!(changelog_list_response.status(), StatusCode::OK);
    let changelog_list: serde_json::Value = response_json(changelog_list_response).await;
    assert_eq!(changelog_list["data"]["versions"][0], "4.1.0");

    let changelog_response = get(router.clone(), "/api/stat/changelog?version=v4.1.0").await;
    assert_eq!(changelog_response.status(), StatusCode::OK);
    let changelog: serde_json::Value = response_json(changelog_response).await;
    assert_eq!(changelog["data"]["content"], "Dashboard parity");

    let update_response = post_json(
        router.clone(),
        "/api/update/do",
        json!({ "version": "v4.1.0", "reboot": false }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update: serde_json::Value = response_json(update_response).await;
    assert_eq!(update["data"]["operation"]["kind"], "project_update");
    assert_eq!(
        update["data"]["operation"]["progress"]["status"],
        "completed"
    );

    let package_response = post_json(
        router.clone(),
        "/api/update/pip-install",
        json!({ "package": "requests==2.32.0" }),
    )
    .await;
    assert_eq!(package_response.status(), StatusCode::OK);
    let package: serde_json::Value = response_json(package_response).await;
    assert_eq!(package["data"]["operation"]["kind"], "package_install");

    let migration_response = post_json(
        router.clone(),
        "/api/update/migration",
        json!({ "platform_id_map": { "aiocqhttp": { "default": "onebot" } } }),
    )
    .await;
    assert_eq!(migration_response.status(), StatusCode::OK);
    let migration: serde_json::Value = response_json(migration_response).await;
    assert_eq!(migration["data"]["operation"]["kind"], "migration");

    let restart_response = post_json(router.clone(), "/api/stat/restart-core", json!({})).await;
    assert_eq!(restart_response.status(), StatusCode::OK);

    let bad_proxy_response = post_json(
        router,
        "/api/stat/test-ghproxy-connection",
        json!({ "proxy_url": "" }),
    )
    .await;
    assert_eq!(bad_proxy_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn management_update_operations_persist_and_capture_executor_failures() {
    let db_path = temp_management_file_path("maintenance-operations.db");
    cleanup_sqlite_files(&db_path);
    let store = Arc::new(SqliteMaintenanceOperationStore::new(
        SqliteJsonStore::open(&db_path).expect("sqlite store should open"),
    ));
    let executor = Arc::new(RecordingMaintenanceExecutor::failure(
        "release download failed",
    ));
    let router = management_router(
        management_state_fixture().with_maintenance(
            ManagementMaintenanceState::new("v4.0.0")
                .with_operation_store(store.clone())
                .with_latest_version("v4.1.0")
                .with_release_executor(executor.clone()),
        ),
    );

    let project_response = post_json(
        router.clone(),
        "/api/management/update/project-plan",
        json!({ "version": "v4.1.0", "reboot": false }),
    )
    .await;
    assert_eq!(project_response.status(), StatusCode::OK);

    let reloaded_router = management_router(
        management_state_fixture().with_maintenance(
            ManagementMaintenanceState::new("v4.0.0")
                .with_operation_store(Arc::new(SqliteMaintenanceOperationStore::new(
                    SqliteJsonStore::open(&db_path).expect("sqlite store should reopen"),
                )))
                .with_release_executor(executor),
        ),
    );
    let persisted = get(
        reloaded_router.clone(),
        "/api/management/update/operations/project-update-v4.1.0",
    )
    .await;
    assert_eq!(persisted.status(), StatusCode::OK);
    let persisted_payload: serde_json::Value = response_json(persisted).await;
    assert_eq!(
        persisted_payload["operation"]["progress"]["status"],
        "running"
    );

    let run_without_confirm = post_json(
        reloaded_router.clone(),
        "/api/management/update/operations/run",
        json!({ "operation_id": "project-update-v4.1.0" }),
    )
    .await;
    assert_eq!(run_without_confirm.status(), StatusCode::OK);
    let unconfirmed: serde_json::Value = response_json(run_without_confirm).await;
    assert_eq!(unconfirmed["operation"]["progress"]["status"], "failed");
    assert_eq!(
        unconfirmed["operation"]["progress"]["error"],
        "maintenance operation requires explicit confirmation"
    );

    let replan_response = post_json(
        reloaded_router.clone(),
        "/api/management/update/project-plan",
        json!({ "version": "v4.1.0", "reboot": false }),
    )
    .await;
    assert_eq!(replan_response.status(), StatusCode::OK);

    let failed_run = post_json(
        reloaded_router.clone(),
        "/api/management/update/operations/run",
        json!({ "operation_id": "project-update-v4.1.0", "confirmed": true }),
    )
    .await;
    assert_eq!(failed_run.status(), StatusCode::OK);
    let failed_payload: serde_json::Value = response_json(failed_run).await;
    assert_eq!(failed_payload["operation"]["progress"]["status"], "failed");
    assert_eq!(
        failed_payload["operation"]["progress"]["error"],
        "release download failed"
    );

    let failed_lookup = get(
        reloaded_router,
        "/api/management/update/operations/project-update-v4.1.0",
    )
    .await;
    let failed_lookup_payload: serde_json::Value = response_json(failed_lookup).await;
    assert_eq!(
        failed_lookup_payload["operation"]["progress"]["error"],
        "release download failed"
    );

    cleanup_sqlite_files(&db_path);
}

#[tokio::test]
async fn local_maintenance_executor_runs_config_and_sqlite_migrations() {
    let root = temp_management_dir_path("maintenance-local");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("migration fixture root should exist");
    let config_path = root.join("config.json");
    let db_path = root.join("main.sqlite");
    fs::write(&config_path, r#"{"webchat_server":{"enabled":true}}"#)
        .expect("legacy config should write");
    cleanup_sqlite_files(&db_path);

    let executor = LocalMaintenanceExecutor::new(root.clone())
        .with_runtime_config_path(config_path.clone())
        .with_sqlite_path(db_path.clone());
    let messages = executor
        .run_migration(MaintenanceMigrationRequest {
            confirmed: true,
            platform_id_map: Default::default(),
        })
        .await
        .expect("migration should run");

    assert!(
        messages
            .iter()
            .any(|message| message.contains("runtime config defaults merged"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("sqlite storage migration applied for main_db v4"))
    );
    let migrated_config: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&config_path).expect("migrated config should read"),
    )
    .expect("migrated config should parse");
    assert!(migrated_config["event_queue_capacity"].is_number());
    assert!(db_path.exists());

    cleanup_sqlite_files(&db_path);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn management_observability_routes_expose_logs_and_traces() {
    let logs = Arc::new(InMemoryLogBuffer::new(8));
    let metrics_path = std::env::temp_dir().join(format!(
        "astrbot-web-management-metrics-{}.jsonl",
        std::process::id()
    ));
    let _ = fs::remove_file(&metrics_path);
    logs.push(LogEntry::new(
        LogLevel::Info,
        LogSource::Dashboard,
        "dashboard ready",
    ))
    .await;
    let trace = TraceEvent {
        span_id: "span-1".to_string(),
        span_name: "pipeline".to_string(),
        action: "process".to_string(),
        message_origin: Some("webchat:user".to_string()),
        sender_name: Some("alice".to_string()),
        message_outline: Some("hello".to_string()),
        fields: vec![
            ("provider".to_string(), "mock".to_string()),
            ("authorization".to_string(), "Bearer secret".to_string()),
        ],
        occurred_at: SystemTime::now(),
        elapsed: None,
    };
    let state = management_state_fixture().with_observability(
        ManagementObservabilityState::new(logs, vec![trace])
            .with_metric_file(metrics_path.clone())
            .with_metrics(vec![
                MetricEvent::platform_message("2026-05-17T08:00:00Z", "webchat", "webchat", 2),
                MetricEvent::llm_response(
                    "2026-05-17T08:00:01Z",
                    "mock-provider",
                    UsageRecord::new(10, 2, 5),
                ),
            ]),
    );
    let router = management_router(state);

    let logs_response = get(router.clone(), "/api/management/logs?limit=4").await;
    assert_eq!(logs_response.status(), StatusCode::OK);
    let logs: serde_json::Value = response_json(logs_response).await;
    assert_eq!(logs["snapshot"]["entries"][0]["message"], "dashboard ready");

    let stream_response = get(
        router.clone(),
        "/api/management/logs/stream?limit=4&max_ticks=1",
    )
    .await;
    assert_eq!(stream_response.status(), StatusCode::OK);
    let stream_body = to_bytes(stream_response.into_body(), usize::MAX)
        .await
        .expect("sse body should read");
    let stream_text = String::from_utf8(stream_body.to_vec()).expect("sse should be utf8");
    assert!(stream_text.contains("event: log"));
    assert!(stream_text.contains("dashboard ready"));

    let trace_response = get(router.clone(), "/api/management/trace").await;
    assert_eq!(trace_response.status(), StatusCode::OK);
    let trace: serde_json::Value = response_json(trace_response).await;
    assert_eq!(trace["events"][0]["span_id"], "span-1");
    assert_eq!(trace["events"][0]["fields"][1][1], "[REDACTED]");
    assert_eq!(trace["settings"]["enabled"], true);

    let legacy_history_response = get(router.clone(), "/api/log-history").await;
    assert_eq!(legacy_history_response.status(), StatusCode::OK);
    let legacy_history: serde_json::Value = response_json(legacy_history_response).await;
    let legacy_rows = legacy_history["data"]["logs"]
        .as_array()
        .expect("legacy logs should be an array");
    assert!(legacy_rows.iter().any(|row| {
        row["type"] == "log" && row["level"] == "INFO" && row["data"] == "dashboard ready"
    }));
    let legacy_trace = legacy_rows
        .iter()
        .find(|row| row["type"] == "trace")
        .expect("legacy history should include trace rows");
    assert_eq!(legacy_trace["span_id"], "span-1");
    assert_eq!(legacy_trace["fields"]["authorization"], "[REDACTED]");

    let trace_settings_response = post_json(
        router.clone(),
        "/api/management/trace/settings",
        json!({
            "enabled": false,
            "capture_message_outline": false,
            "max_events": 12,
            "redact_fields": ["secret", "authorization", ""]
        }),
    )
    .await;
    assert_eq!(trace_settings_response.status(), StatusCode::OK);
    let trace_settings: serde_json::Value = response_json(trace_settings_response).await;
    assert_eq!(trace_settings["enabled"], false);
    assert_eq!(trace_settings["capture_message_outline"], false);
    assert_eq!(trace_settings["max_events"], 12);
    assert_eq!(
        trace_settings["redact_fields"],
        json!(["secret", "authorization"])
    );

    let stats_response = get(router.clone(), "/api/management/stats").await;
    assert_eq!(stats_response.status(), StatusCode::OK);
    let stats: serde_json::Value = response_json(stats_response).await;
    assert_eq!(stats["total_messages"], 2);
    assert_eq!(stats["total_llm_calls"], 1);
    assert_eq!(stats["total_tokens"], 17);
    assert_eq!(stats["platform_counts"][0]["platform_id"], "webchat");
    assert_eq!(stats["provider_usage"][0]["provider_id"], "mock-provider");

    let push_metric_response = post_json(
        router,
        "/api/management/stats/push",
        json!(MetricEvent::platform_message(
            "2026-05-17T09:00:00Z",
            "webchat",
            "webchat",
            3
        )),
    )
    .await;
    assert_eq!(push_metric_response.status(), StatusCode::OK);
    let pushed: serde_json::Value = response_json(push_metric_response).await;
    assert_eq!(pushed["total_messages"], 5);

    let persisted = fs::read_to_string(&metrics_path).expect("metrics jsonl should persist");
    assert!(persisted.contains("2026-05-17T09:00:00Z"));
    let reloaded =
        ManagementObservabilityState::new(Arc::new(InMemoryLogBuffer::new(1)), Vec::new())
            .with_metric_file(metrics_path.clone());
    let reloaded_metrics = reloaded.metrics().expect("metrics should load");
    assert_eq!(reloaded_metrics.len(), 1);
    assert_eq!(reloaded_metrics[0].platform_id.as_deref(), Some("webchat"));
    let _ = fs::remove_file(&metrics_path);
}

#[tokio::test]
async fn management_observability_persists_logs_trace_settings_and_sqlite_stats() {
    let db_path = temp_management_file_path("observability-stats.db");
    cleanup_sqlite_files(&db_path);
    let log_path = temp_management_file_path("observability-log.jsonl");
    let trace_settings_path = temp_management_file_path("observability-trace-settings.json");
    let _ = fs::remove_file(&log_path);
    let _ = fs::remove_file(&trace_settings_path);
    let storage = Arc::new(SqliteStorage::open(&db_path).expect("sqlite should open"));
    storage
        .increment_platform_stats(PlatformStatsRecord::new(
            "2026-05-17T08:00:00Z",
            "webchat",
            "webchat",
            4,
        ))
        .await
        .expect("platform stats should persist");
    storage
        .increment_platform_stats(PlatformStatsRecord::new(
            "2026-05-17T08:00:00Z",
            "mock",
            "mock",
            6,
        ))
        .await
        .expect("platform stats should persist");

    let observability =
        ManagementObservabilityState::new(Arc::new(InMemoryLogBuffer::new(8)), Vec::new())
            .with_log_file(log_path.clone())
            .await
            .expect("log store should open")
            .with_trace_settings_file(trace_settings_path.clone())
            .expect("trace settings store should open");
    let router =
        management_router(management_state_fixture().with_observability(observability.clone()));

    let push_log_response = post_json(
        router.clone(),
        "/api/management/logs/push",
        json!(LogEntry::new(
            LogLevel::Info,
            LogSource::Runtime,
            "runtime booted with api_key=secret"
        )),
    )
    .await;
    assert_eq!(push_log_response.status(), StatusCode::OK);
    let pushed_logs: serde_json::Value = response_json(push_log_response).await;
    let pushed_id = pushed_logs["snapshot"]["entries"][0]["id"]
        .as_u64()
        .expect("log id");

    let legacy_history = get(router.clone(), "/api/log-history").await;
    assert_eq!(legacy_history.status(), StatusCode::OK);
    let legacy_history: serde_json::Value = response_json(legacy_history).await;
    assert_eq!(legacy_history["status"], "ok");
    assert_eq!(legacy_history["data"]["logs"][0]["type"], "log");
    assert_eq!(legacy_history["data"]["logs"][0]["level"], "INFO");
    assert_eq!(
        legacy_history["data"]["logs"][0]["data"],
        "runtime booted with api_key=[REDACTED]"
    );
    assert_eq!(
        legacy_history["data"]["logs"][0]["message"],
        "runtime booted with api_key=[REDACTED]"
    );

    let trace_update = post_json(
        router.clone(),
        "/api/trace/settings",
        json!({ "trace_enable": false }),
    )
    .await;
    assert_eq!(trace_update.status(), StatusCode::OK);
    let trace_update: serde_json::Value = response_json(trace_update).await;
    assert_eq!(trace_update["data"]["trace_enable"], false);

    let reloaded_observability =
        ManagementObservabilityState::new(Arc::new(InMemoryLogBuffer::new(8)), Vec::new())
            .with_log_file(log_path.clone())
            .await
            .expect("log store should reload")
            .with_trace_settings_file(trace_settings_path.clone())
            .expect("trace settings store should reload")
            .with_platform_stats_repository(storage.clone());
    let reloaded_router = management_router(
        management_state_fixture()
            .with_observability(reloaded_observability)
            .with_sqlite_storage_path(&db_path)
            .expect("sqlite-backed management state should build"),
    );

    let logs_response = get(reloaded_router.clone(), "/api/management/logs?limit=8").await;
    assert_eq!(logs_response.status(), StatusCode::OK);
    let logs: serde_json::Value = response_json(logs_response).await;
    assert_eq!(
        logs["snapshot"]["entries"][0]["message"],
        "runtime booted with api_key=[REDACTED]"
    );
    let persisted_log = fs::read_to_string(&log_path).expect("log jsonl should persist");
    assert!(persisted_log.contains("api_key=[REDACTED]"));
    assert!(!persisted_log.contains("api_key=secret"));

    let replay_response = get(
        reloaded_router.clone(),
        &format!(
            "/api/live-log?last_event_id={}&limit=8&max_ticks=1",
            pushed_id.saturating_sub(1)
        ),
    )
    .await;
    assert_eq!(replay_response.status(), StatusCode::OK);
    let replay_body = to_bytes(replay_response.into_body(), usize::MAX)
        .await
        .expect("sse body should read");
    let replay_text = String::from_utf8(replay_body.to_vec()).expect("sse should be utf8");
    assert!(!replay_text.contains("event: log"));
    assert!(replay_text.contains("data:"));
    assert!(replay_text.contains("\"type\":\"log\""));
    assert!(replay_text.contains("runtime booted"));
    assert!(replay_text.contains("[REDACTED]"));

    let trace_response = get(reloaded_router.clone(), "/api/trace/settings").await;
    assert_eq!(trace_response.status(), StatusCode::OK);
    let trace: serde_json::Value = response_json(trace_response).await;
    assert_eq!(trace["data"]["trace_enable"], false);

    let stats_response = get(reloaded_router.clone(), "/api/management/stats").await;
    assert_eq!(stats_response.status(), StatusCode::OK);
    let stats: serde_json::Value = response_json(stats_response).await;
    assert_eq!(stats["total_messages"], 10);
    assert!(
        stats["platform_counts"]
            .as_array()
            .expect("platform counts")
            .iter()
            .any(|platform| platform["platform_id"] == "webchat" && platform["count"] == 4)
    );

    let legacy_stats_response = get(reloaded_router, "/api/stat/get").await;
    assert_eq!(legacy_stats_response.status(), StatusCode::OK);
    let legacy_stats: serde_json::Value = response_json(legacy_stats_response).await;
    assert_eq!(legacy_stats["data"]["message_count"], 10);
    let legacy_platform_stats_response = get(
        management_router(
            management_state_fixture()
                .with_observability(
                    ManagementObservabilityState::new(
                        Arc::new(InMemoryLogBuffer::new(8)),
                        Vec::new(),
                    )
                    .with_platform_stats_repository(storage),
                )
                .with_config_service(RuntimeConfigService::new(temp_management_config_path(
                    "observability-platform-stats",
                ))),
        ),
        "/api/platform/stats",
    )
    .await;
    assert_eq!(legacy_platform_stats_response.status(), StatusCode::OK);
    let legacy_platform_stats: serde_json::Value =
        response_json(legacy_platform_stats_response).await;
    let mock = legacy_platform_stats["data"]["platforms"]
        .as_array()
        .expect("platforms")
        .iter()
        .find(|platform| platform["id"] == "mock")
        .expect("mock platform");
    assert_eq!(mock["message_count"], 6);

    cleanup_sqlite_files(&db_path);
    let _ = fs::remove_file(log_path);
    let _ = fs::remove_file(trace_settings_path);
}

#[tokio::test]
async fn management_persona_routes_list_upsert_and_resolve_profiles() {
    let manager = Arc::new(PersonaManager::with_repository(
        Arc::new(InMemoryPersonaRepository::new()),
        PersonaProfile::new("default", "be helpful"),
    ));
    manager
        .upsert_folder(PersonaFolder::new("root", "Root"))
        .await
        .expect("folder should save");
    manager
        .upsert_persona(PersonaProfile::new("support", "be concise").with_folder_id("root"))
        .await
        .expect("persona should save");
    let state = management_state_fixture().with_personas(ManagementPersonaState::new(manager));
    let router = management_router(state);

    let list_response = post_json(
        router.clone(),
        "/api/management/personas",
        json!({ "folder_id": "root" }),
    )
    .await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert_eq!(list["personas"][0]["id"], "support");

    let upsert_response = post_json(
        router.clone(),
        "/api/management/personas/upsert",
        json!({
            "id": "analyst",
            "system_prompt": "be rigorous",
            "folder_id": "root",
            "sort_order": 5,
            "tools": ["astr_kb_search"],
            "begin_dialogs": [{ "role": "user", "content": "hello" }]
        }),
    )
    .await;
    assert_eq!(upsert_response.status(), StatusCode::OK);

    let clone_response = post_json(
        router.clone(),
        "/api/management/personas/clone",
        json!({
            "source_id": "analyst",
            "new_id": "analyst-copy",
            "folder_id": "root"
        }),
    )
    .await;
    assert_eq!(clone_response.status(), StatusCode::OK);
    let cloned: serde_json::Value = response_json(clone_response).await;
    assert_eq!(cloned["persona"]["id"], "analyst-copy");
    assert_eq!(cloned["persona"]["system_prompt"], "be rigorous");

    let move_response = post_json(
        router.clone(),
        "/api/management/personas/move",
        json!({
            "id": "analyst-copy",
            "folder_id": null,
            "sort_order": 0
        }),
    )
    .await;
    assert_eq!(move_response.status(), StatusCode::OK);
    let moved: serde_json::Value = response_json(move_response).await;
    assert_eq!(moved["persona"]["id"], "analyst-copy");
    assert!(moved["persona"]["folder_id"].is_null());

    let child_folder_response = post_json(
        router.clone(),
        "/api/management/personas/folders/upsert",
        json!({
            "id": "child",
            "name": "Child",
            "parent_id": "root",
            "sort_order": 2
        }),
    )
    .await;
    assert_eq!(child_folder_response.status(), StatusCode::OK);

    let move_folder_response = post_json(
        router.clone(),
        "/api/management/personas/folders/move",
        json!({
            "id": "child",
            "parent_id": null,
            "sort_order": 0
        }),
    )
    .await;
    assert_eq!(move_folder_response.status(), StatusCode::OK);
    let moved_folder: serde_json::Value = response_json(move_folder_response).await;
    assert_eq!(moved_folder["folder"]["id"], "child");
    assert!(moved_folder["folder"]["parent_id"].is_null());

    let reorder_response = post_json(
        router.clone(),
        "/api/management/personas/reorder",
        json!({
            "persona_ids": ["analyst-copy", "support", "analyst"],
            "folder_ids": ["child", "root"]
        }),
    )
    .await;
    assert_eq!(reorder_response.status(), StatusCode::OK);
    let reordered: serde_json::Value = response_json(reorder_response).await;
    assert_eq!(reordered["personas"][0]["id"], "analyst-copy");
    assert_eq!(reordered["folders"][0]["id"], "child");

    let delete_response = post_json(
        router.clone(),
        "/api/management/personas/delete",
        json!({ "id": "analyst-copy" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_response).await;
    assert_eq!(deleted["deleted"], true);
    assert!(
        !deleted["personas"]
            .as_array()
            .expect("personas")
            .iter()
            .any(|persona| persona["id"] == "analyst-copy")
    );

    let delete_folder_response = post_json(
        router.clone(),
        "/api/management/personas/folders/delete",
        json!({ "id": "child" }),
    )
    .await;
    assert_eq!(delete_folder_response.status(), StatusCode::OK);
    let deleted_folder: serde_json::Value = response_json(delete_folder_response).await;
    assert_eq!(deleted_folder["deleted"], true);

    let resolve_response = post_json(
        router,
        "/api/management/personas/resolve",
        json!({ "forced_persona_id": "analyst" }),
    )
    .await;
    assert_eq!(resolve_response.status(), StatusCode::OK);
    let resolved: serde_json::Value = response_json(resolve_response).await;
    assert_eq!(resolved["persona_id"], "analyst");
    assert_eq!(resolved["source"], "forced_session");
}

#[tokio::test]
async fn source_persona_facade_matches_dashboard_folder_tree_and_form_shapes() {
    let manager = Arc::new(PersonaManager::with_repository(
        Arc::new(InMemoryPersonaRepository::new()),
        PersonaProfile::new("default", "be helpful"),
    ));
    let router = management_router(
        management_state_fixture().with_personas(ManagementPersonaState::new(manager)),
    );

    let root_folder_response = post_json(
        router.clone(),
        "/api/persona/folder/create",
        json!({
            "folder_id": "ops",
            "name": "Ops",
            "description": "Operations",
            "sort_order": 1
        }),
    )
    .await;
    assert_eq!(root_folder_response.status(), StatusCode::OK);

    let child_folder_response = post_json(
        router.clone(),
        "/api/persona/folder/create",
        json!({
            "folder_id": "incident",
            "name": "Incident",
            "parent_id": "ops",
            "sort_order": 2
        }),
    )
    .await;
    assert_eq!(child_folder_response.status(), StatusCode::OK);

    let create_response = post_json(
        router.clone(),
        "/api/persona/create",
        json!({
            "persona_id": "support",
            "system_prompt": "Help safely",
            "custom_error_message": "Cannot comply",
            "begin_dialogs": ["hello", "hi"],
            "tools": null,
            "skills": ["writer"],
            "folder_id": "ops",
            "sort_order": 3
        }),
    )
    .await;
    assert_eq!(create_response.status(), StatusCode::OK);
    let created: serde_json::Value = response_json(create_response).await;
    assert_eq!(created["data"]["persona"]["begin_dialogs"][0], "hello");
    assert!(created["data"]["persona"]["tools"].is_null());
    assert_eq!(created["data"]["persona"]["skills"][0], "writer");

    let tree_response = get(router.clone(), "/api/persona/folder/tree").await;
    assert_eq!(tree_response.status(), StatusCode::OK);
    let tree: serde_json::Value = response_json(tree_response).await;
    assert_eq!(tree["data"][0]["folder_id"], "ops");
    assert_eq!(tree["data"][0]["children"][0]["folder_id"], "incident");

    let list_response = get(router.clone(), "/api/persona/list?folder_id=ops").await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert_eq!(list["data"][0]["persona_id"], "support");
    assert_eq!(list["data"][0]["begin_dialogs"][1], "hi");
    assert!(list["data"][0]["tools"].is_null());

    let clone_response = post_json(
        router.clone(),
        "/api/persona/clone",
        json!({
            "source_persona_id": "support",
            "new_persona_id": "support_copy"
        }),
    )
    .await;
    assert_eq!(clone_response.status(), StatusCode::OK);
    let cloned: serde_json::Value = response_json(clone_response).await;
    assert_eq!(cloned["data"]["persona"]["persona_id"], "support_copy");

    let move_response = post_json(
        router.clone(),
        "/api/persona/move",
        json!({
            "persona_id": "support_copy",
            "folder_id": "incident"
        }),
    )
    .await;
    assert_eq!(move_response.status(), StatusCode::OK);

    let reorder_response = post_json(
        router.clone(),
        "/api/persona/reorder",
        json!({
            "items": [
                { "id": "support_copy", "type": "persona", "sort_order": 0 },
                { "id": "support", "type": "persona", "sort_order": 1 },
                { "id": "incident", "type": "folder", "sort_order": 0 },
                { "id": "ops", "type": "folder", "sort_order": 1 }
            ]
        }),
    )
    .await;
    assert_eq!(reorder_response.status(), StatusCode::OK);

    let delete_response = post_json(
        router,
        "/api/persona/delete",
        json!({ "persona_id": "support_copy" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn management_cron_routes_manage_scheduler_jobs() {
    let scheduler = Arc::new(CronScheduler::new(
        Arc::new(InMemoryCronJobRepository::new()),
        Arc::new(DueCronScheduleDriver::new()),
        Arc::new(ProactiveAgentWakeService::new(
            Arc::new(RecordingCronEventSink::new()),
            Arc::new(ManagementNoopMessageSink),
        )),
    ));
    scheduler
        .add_job(CronJob::active_agent(
            "daily",
            "Daily",
            CronJobSchedule::cron("0 8 * * *"),
            ActiveAgentCronPayload::new("webchat:demo", "hello"),
        ))
        .await
        .expect("job should save");
    let state = management_state_fixture().with_cron(ManagementCronState::new(scheduler));
    let router = management_router(state);

    let list_response = post_json(router.clone(), "/api/management/cron/jobs", json!({})).await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert_eq!(list["jobs"][0]["job_id"], "daily");
    assert_eq!(list["state"], "stopped");

    let start_response = post_json(router.clone(), "/api/management/cron/start", json!({})).await;
    assert_eq!(start_response.status(), StatusCode::OK);

    let upsert_response = post_json(
        router.clone(),
        "/api/management/cron/jobs/upsert",
        json!({
            "job": {
                "job_id": "run-once",
                "name": "Run once",
                "kind": "active_agent",
                "schedule": {
                    "spec": { "run_once": { "run_at": "2026-05-17T00:00:00Z" } },
                    "timezone": "UTC"
                },
                "payload": { "session": "webchat:demo", "note": "wake once" },
                "description": null,
                "enabled": true,
                "persistent": true,
                "status": "scheduled",
                "last_error": null
            }
        }),
    )
    .await;
    assert_eq!(upsert_response.status(), StatusCode::OK);

    let tick_response = post_json(
        router.clone(),
        "/api/management/cron/tick",
        json!({ "now_unix": 1778976000u64 }),
    )
    .await;
    assert_eq!(tick_response.status(), StatusCode::OK);
    let tick: serde_json::Value = response_json(tick_response).await;
    assert_eq!(tick["report"]["due_count"], 1);
    assert_eq!(tick["report"]["ran_job_ids"][0], "run-once");

    let run_response = post_json(
        router.clone(),
        "/api/management/cron/jobs/run",
        json!({ "job_id": "daily" }),
    )
    .await;
    assert_eq!(run_response.status(), StatusCode::OK);

    let delete_response = post_json(
        router,
        "/api/management/cron/jobs/delete",
        json!({ "job_id": "daily" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_response).await;
    assert_eq!(deleted["deleted"], true);
}

#[tokio::test]
async fn source_cron_facade_matches_dashboard_job_shapes_and_mutations() {
    let event_sink = Arc::new(RecordingCronEventSink::new());
    let scheduler = Arc::new(CronScheduler::new(
        Arc::new(InMemoryCronJobRepository::new()),
        Arc::new(DueCronScheduleDriver::new()),
        Arc::new(ProactiveAgentWakeService::new(
            event_sink.clone(),
            Arc::new(ManagementNoopMessageSink),
        )),
    ));
    scheduler.start().await.expect("scheduler should start");
    let state = management_state_fixture().with_cron(ManagementCronState::new(scheduler));
    let router = management_router(state);

    let create_response = post_json(
        router.clone(),
        "/api/cron/jobs",
        json!({
            "name": "Ops Wake",
            "session": "webchat:ops-room:group",
            "note": "Check incident status",
            "cron_expression": "0 9 * * *",
            "timezone": "Asia/Shanghai",
            "persona_id": "support",
            "provider_id": "openai/gpt-4.1-mini",
            "enabled": true
        }),
    )
    .await;
    assert_eq!(create_response.status(), StatusCode::OK);
    let created: serde_json::Value = response_json(create_response).await;
    assert_eq!(created["status"], "ok");
    let job_id = created["data"]["job_id"]
        .as_str()
        .expect("job id should exist")
        .to_string();
    assert_eq!(created["data"]["job_type"], "active_agent");
    assert_eq!(created["data"]["session"], "webchat:ops-room:group");
    assert_eq!(created["data"]["cron_expression"], "0 9 * * *");
    assert_eq!(created["data"]["timezone"], "Asia/Shanghai");
    assert_eq!(created["data"]["note"], "Check incident status");
    assert_eq!(created["data"]["payload"]["persona_id"], "support");
    assert_eq!(
        created["data"]["payload"]["provider_id"],
        "openai/gpt-4.1-mini"
    );
    assert_eq!(created["data"]["run_once"], false);

    let toggle_response = patch_json(
        router.clone(),
        &format!("/api/cron/jobs/{job_id}"),
        json!({ "enabled": false }),
    )
    .await;
    assert_eq!(toggle_response.status(), StatusCode::OK);
    let toggled: serde_json::Value = response_json(toggle_response).await;
    assert_eq!(toggled["data"]["enabled"], false);
    assert_eq!(toggled["data"]["session"], "webchat:ops-room:group");
    assert_eq!(toggled["data"]["cron_expression"], "0 9 * * *");

    let retoggle_response = patch_json(
        router.clone(),
        &format!("/api/cron/jobs/{job_id}"),
        json!({ "enabled": true }),
    )
    .await;
    assert_eq!(retoggle_response.status(), StatusCode::OK);

    let list_response = get(router.clone(), "/api/cron/jobs?type=active_agent").await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert!(
        list["data"]
            .as_array()
            .expect("cron jobs")
            .iter()
            .any(|job| job["job_id"] == job_id
                && job["job_type"] == "active_agent"
                && job["session"] == "webchat:ops-room:group")
    );

    let once_response = post_json(
        router.clone(),
        "/api/cron/jobs",
        json!({
            "name": "Run Once",
            "session": "webchat:ops-room",
            "note": "Wake once",
            "run_once": true,
            "run_at": "2026-05-20T08:00:00+08:00"
        }),
    )
    .await;
    assert_eq!(once_response.status(), StatusCode::OK);
    let once: serde_json::Value = response_json(once_response).await;
    assert_eq!(once["data"]["run_once"], true);
    assert_eq!(once["data"]["run_at"], "2026-05-20T08:00:00+08:00");
    assert_eq!(once["data"]["next_run_time"], "2026-05-20T08:00:00+08:00");

    let run_response = post_json(
        router.clone(),
        &format!("/api/cron/jobs/{job_id}/run"),
        json!({}),
    )
    .await;
    assert_eq!(run_response.status(), StatusCode::OK);
    let run: serde_json::Value = response_json(run_response).await;
    assert_eq!(run["status"], "ok");
    assert_eq!(event_sink.events().len(), 1);

    let delete_response = delete(router, &format!("/api/cron/jobs/{job_id}")).await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_response).await;
    assert_eq!(deleted["status"], "ok");
}

#[tokio::test]
async fn management_conversation_routes_manage_directory_records() {
    let state = management_state_fixture()
        .with_conversations(ManagementConversationState::new(ConversationService::new()));
    let router = management_router(state);

    let create_response = post_json(
        router.clone(),
        "/api/management/conversations/upsert",
        json!({
            "platform_id": "webchat",
            "conversation_id": "conversation-1",
            "title": "General",
            "persona_id": "support",
            "set_current": true
        }),
    )
    .await;
    assert_eq!(create_response.status(), StatusCode::OK);
    let created: serde_json::Value = response_json(create_response).await;
    assert_eq!(created["conversation"]["conversation_id"], "conversation-1");
    assert_eq!(created["conversation"]["current"], true);

    let second_response = post_json(
        router.clone(),
        "/api/management/conversations/upsert",
        json!({
            "platform_id": "webchat",
            "conversation_id": "conversation-2",
            "title": "Scratch"
        }),
    )
    .await;
    assert_eq!(second_response.status(), StatusCode::OK);

    let rename_response = post_json(
        router.clone(),
        "/api/management/conversations/rename",
        json!({
            "platform_id": "webchat",
            "conversation_id": "conversation-1",
            "title": "Renamed"
        }),
    )
    .await;
    assert_eq!(rename_response.status(), StatusCode::OK);
    let renamed: serde_json::Value = response_json(rename_response).await;
    assert_eq!(renamed["conversation"]["title"], "Renamed");
    assert_eq!(renamed["conversation"]["persona_id"], "support");

    let list_response = post_json(
        router.clone(),
        "/api/management/conversations",
        json!({ "platform_id": "webchat" }),
    )
    .await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert_eq!(
        list["conversations"]
            .as_array()
            .expect("conversations")
            .len(),
        2
    );
    assert_eq!(
        list["conversations"][0]["conversation_id"],
        "conversation-1"
    );

    let current_response = post_json(
        router.clone(),
        "/api/management/conversations/current",
        json!({ "platform_id": "webchat", "conversation_id": "conversation-2" }),
    )
    .await;
    assert_eq!(current_response.status(), StatusCode::OK);
    let current: serde_json::Value = response_json(current_response).await;
    assert_eq!(current["conversation"]["conversation_id"], "conversation-2");
    assert_eq!(current["conversation"]["current"], true);

    let batch_delete_response = post_json(
        router,
        "/api/management/conversations/batch-delete",
        json!({
            "platform_id": "webchat",
            "conversation_ids": ["conversation-1", "missing"]
        }),
    )
    .await;
    assert_eq!(batch_delete_response.status(), StatusCode::OK);
    let batch: serde_json::Value = response_json(batch_delete_response).await;
    assert_eq!(batch["deleted_count"], 1);
    assert_eq!(batch["deleted_ids"][0], "conversation-1");
    assert_eq!(batch["missing_ids"][0], "missing");
}

#[tokio::test]
async fn source_conversation_routes_cover_list_detail_update_history_export_and_delete() {
    let service = ConversationService::new();
    service
        .upsert(
            astrbot_conversation::ConversationRecord::new("telegram", "conversation-a")
                .with_user_id("telegram:GroupMessage:room-1")
                .with_title("Ops room")
                .with_persona_id("support")
                .with_history(
                    serde_json::json!([
                        { "role": "user", "content": "hello" },
                        { "role": "assistant", "content": [{ "type": "text", "text": "world" }] }
                    ])
                    .to_string(),
                )
                .with_created_at(10)
                .with_updated_at(20),
        )
        .await
        .expect("conversation should upsert");
    service
        .upsert(
            astrbot_conversation::ConversationRecord::new("webchat", "conversation-b")
                .with_user_id("webchat:FriendMessage:dashboard")
                .with_title("Excluded")
                .with_history("[]")
                .with_created_at(30)
                .with_updated_at(30),
        )
        .await
        .expect("conversation should upsert");
    let state =
        management_state_fixture().with_conversations(ManagementConversationState::new(service));
    let router = management_router(state);

    let list_response = get(
        router.clone(),
        "/api/conversation/list?page=1&page_size=10&platforms=telegram&message_types=GroupMessage&search=ops",
    )
    .await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert_eq!(list["status"], "ok");
    assert_eq!(list["data"]["pagination"]["total"], 1);
    assert_eq!(list["data"]["conversations"][0]["cid"], "conversation-a");
    assert_eq!(
        list["data"]["conversations"][0]["user_id"],
        "telegram:GroupMessage:room-1"
    );

    let detail_response = post_json(
        router.clone(),
        "/api/conversation/detail",
        json!({ "user_id": "telegram:GroupMessage:room-1", "cid": "conversation-a" }),
    )
    .await;
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail: serde_json::Value = response_json(detail_response).await;
    assert_eq!(detail["data"]["title"], "Ops room");
    assert!(
        detail["data"]["history"]
            .as_str()
            .expect("history string")
            .contains("hello")
    );

    let update_response = post_json(
        router.clone(),
        "/api/conversation/update",
        json!({
            "user_id": "telegram:GroupMessage:room-1",
            "cid": "conversation-a",
            "title": "Renamed room",
            "persona_id": "ops"
        }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);

    let history_response = post_json(
        router.clone(),
        "/api/conversation/update_history",
        json!({
            "user_id": "telegram:GroupMessage:room-1",
            "cid": "conversation-a",
            "history": [{ "role": "user", "content": "edited" }]
        }),
    )
    .await;
    assert_eq!(history_response.status(), StatusCode::OK);

    let export_response = post_json(
        router.clone(),
        "/api/conversation/export",
        json!({
            "conversations": [
                { "user_id": "telegram:GroupMessage:room-1", "cid": "conversation-a" }
            ]
        }),
    )
    .await;
    assert_eq!(export_response.status(), StatusCode::OK);
    let export_bytes = axum::body::to_bytes(export_response.into_body(), usize::MAX)
        .await
        .expect("export body");
    let export_text = String::from_utf8(export_bytes.to_vec()).expect("utf8 export");
    assert!(export_text.contains("\"cid\":\"conversation-a\""));
    assert!(export_text.contains("\"edited\""));

    let delete_response = post_json(
        router.clone(),
        "/api/conversation/delete",
        json!({
            "conversations": [
                { "user_id": "telegram:GroupMessage:room-1", "cid": "conversation-a" },
                { "user_id": "telegram:GroupMessage:room-1", "cid": "missing" }
            ]
        }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_response).await;
    assert_eq!(deleted["data"]["deleted_count"], 1);
    assert_eq!(deleted["data"]["failed_count"], 1);
}

#[tokio::test]
async fn management_chat_project_routes_enforce_creator_ownership() {
    let repository = Arc::new(InMemoryChatProjectRepository::new());
    repository
        .upsert_platform_session(
            PlatformSessionRecord::new("session-alice", "webchat", "alice", "2026-05-17T00:00:01Z")
                .with_updated_at("2026-05-17T00:00:01Z")
                .with_display_name("Alice chat"),
        )
        .await
        .expect("session should store");
    repository
        .upsert_platform_session(PlatformSessionRecord::new(
            "session-bob",
            "webchat",
            "bob",
            "2026-05-17T00:00:02Z",
        ))
        .await
        .expect("session should store");
    let state = management_state_fixture().with_chat_projects(ManagementChatProjectState::new(
        ChatProjectService::new(repository),
    ));
    let router = management_router(state);

    let create_response = post_json(
        router.clone(),
        "/api/management/chat-projects/create",
        json!({
            "creator": "alice",
            "title": "Research",
            "emoji": "folder",
            "description": "Project notes",
            "now": "2026-05-17T00:00:00Z"
        }),
    )
    .await;
    assert_eq!(create_response.status(), StatusCode::OK);
    let created: serde_json::Value = response_json(create_response).await;
    let project_id = created["project"]["project_id"]
        .as_str()
        .expect("project id")
        .to_string();

    let forbidden_get = post_json(
        router.clone(),
        "/api/management/chat-projects/get",
        json!({ "actor": "bob", "project_id": project_id }),
    )
    .await;
    assert_eq!(forbidden_get.status(), StatusCode::FORBIDDEN);

    let forbidden_membership = post_json(
        router.clone(),
        "/api/management/chat-projects/add-session",
        json!({
            "actor": "alice",
            "project_id": project_id,
            "session_id": "session-bob"
        }),
    )
    .await;
    assert_eq!(forbidden_membership.status(), StatusCode::FORBIDDEN);

    let upsert_session_response = post_json(
        router.clone(),
        "/api/management/chat-projects/sessions/upsert",
        json!({
            "session_id": "session-dashboard",
            "platform_id": "webchat",
            "creator": "alice",
            "display_name": "Dashboard chat",
            "is_group": false,
            "now": "2026-05-17T00:00:03Z"
        }),
    )
    .await;
    assert_eq!(upsert_session_response.status(), StatusCode::OK);
    let session: serde_json::Value = response_json(upsert_session_response).await;
    assert_eq!(session["session"]["session_id"], "session-dashboard");
    assert_eq!(session["session"]["display_name"], "Dashboard chat");

    let add_response = post_json(
        router.clone(),
        "/api/management/chat-projects/add-session",
        json!({
            "actor": "alice",
            "project_id": project_id,
            "session_id": "session-alice"
        }),
    )
    .await;
    assert_eq!(add_response.status(), StatusCode::OK);

    let sessions_response = post_json(
        router.clone(),
        "/api/management/chat-projects/sessions",
        json!({ "actor": "alice", "project_id": project_id }),
    )
    .await;
    assert_eq!(sessions_response.status(), StatusCode::OK);
    let sessions: serde_json::Value = response_json(sessions_response).await;
    assert_eq!(sessions["sessions"][0]["session_id"], "session-alice");
    assert_eq!(sessions["sessions"][0]["display_name"], "Alice chat");

    let update_response = post_json(
        router.clone(),
        "/api/management/chat-projects/update",
        json!({
            "actor": "alice",
            "project_id": project_id,
            "title": "Research Updated",
            "emoji": "notes",
            "description": "Updated project notes",
            "now": "2026-05-17T00:00:04Z"
        }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated_get = post_json(
        router.clone(),
        "/api/management/chat-projects/get",
        json!({ "actor": "alice", "project_id": project_id }),
    )
    .await;
    assert_eq!(updated_get.status(), StatusCode::OK);
    let updated: serde_json::Value = response_json(updated_get).await;
    assert_eq!(updated["project"]["title"], "Research Updated");

    let remove_response = post_json(
        router.clone(),
        "/api/management/chat-projects/remove-session",
        json!({
            "actor": "alice",
            "project_id": project_id,
            "session_id": "session-alice"
        }),
    )
    .await;
    assert_eq!(remove_response.status(), StatusCode::OK);
    let empty_sessions_response = post_json(
        router.clone(),
        "/api/management/chat-projects/sessions",
        json!({ "actor": "alice", "project_id": project_id }),
    )
    .await;
    assert_eq!(empty_sessions_response.status(), StatusCode::OK);
    let empty_sessions: serde_json::Value = response_json(empty_sessions_response).await;
    assert_eq!(
        empty_sessions["sessions"]
            .as_array()
            .expect("sessions")
            .len(),
        0
    );

    let delete_response = post_json(
        router.clone(),
        "/api/management/chat-projects/delete",
        json!({ "actor": "alice", "project_id": project_id }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let missing_get = post_json(
        router,
        "/api/management/chat-projects/get",
        json!({ "actor": "alice", "project_id": project_id }),
    )
    .await;
    assert_eq!(missing_get.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn source_compatible_chat_project_facade_covers_crud_and_membership() {
    let repository = Arc::new(InMemoryChatProjectRepository::new());
    repository
        .upsert_platform_session(
            PlatformSessionRecord::new(
                "source-session-1",
                "webchat",
                "guest",
                "2026-05-19T00:00:01Z",
            )
            .with_updated_at("2026-05-19T00:00:01Z")
            .with_display_name("Source Session"),
        )
        .await
        .expect("session should store");
    let state = management_state_fixture().with_chat_projects(ManagementChatProjectState::new(
        ChatProjectService::new(repository),
    ));
    let router = management_router(state);

    let create_response = post_json(
        router.clone(),
        "/api/chatui_project/create",
        json!({
            "title": "Source Project",
            "emoji": "S",
            "description": "Source facade"
        }),
    )
    .await;
    assert_eq!(create_response.status(), StatusCode::OK);
    let created: serde_json::Value = response_json(create_response).await;
    assert_eq!(created["status"], "ok");
    let project_id = created["data"]["project_id"]
        .as_str()
        .expect("project id")
        .to_string();

    let list_response = get(router.clone(), "/api/chatui_project/list").await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed: serde_json::Value = response_json(list_response).await;
    assert_eq!(listed["status"], "ok");
    assert!(
        listed["data"]
            .as_array()
            .expect("project list")
            .iter()
            .any(|project| project["project_id"] == project_id
                && project["title"] == "Source Project")
    );

    let get_response = get(
        router.clone(),
        &format!("/api/chatui_project/get?project_id={project_id}"),
    )
    .await;
    assert_eq!(get_response.status(), StatusCode::OK);
    let got: serde_json::Value = response_json(get_response).await;
    assert_eq!(got["data"]["description"], "Source facade");

    let update_response = post_json(
        router.clone(),
        "/api/chatui_project/update",
        json!({
            "project_id": project_id,
            "title": "Updated Source Project",
            "emoji": "U",
            "description": "Updated facade"
        }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated_response = get(
        router.clone(),
        &format!("/api/chatui_project/get?project_id={project_id}"),
    )
    .await;
    let updated: serde_json::Value = response_json(updated_response).await;
    assert_eq!(updated["data"]["title"], "Updated Source Project");
    assert_eq!(updated["data"]["emoji"], "U");

    let add_session_response = post_json(
        router.clone(),
        "/api/chatui_project/add_session",
        json!({
            "project_id": project_id,
            "session_id": "source-session-1"
        }),
    )
    .await;
    assert_eq!(add_session_response.status(), StatusCode::OK);
    let sessions_response = get(
        router.clone(),
        &format!("/api/chatui_project/get_sessions?project_id={project_id}"),
    )
    .await;
    assert_eq!(sessions_response.status(), StatusCode::OK);
    let sessions: serde_json::Value = response_json(sessions_response).await;
    assert_eq!(sessions["data"][0]["session_id"], "source-session-1");
    assert_eq!(sessions["data"][0]["display_name"], "Source Session");

    let remove_session_response = post_json(
        router.clone(),
        "/api/chatui_project/remove_session",
        json!({ "session_id": "source-session-1" }),
    )
    .await;
    assert_eq!(remove_session_response.status(), StatusCode::OK);
    let empty_sessions_response = get(
        router.clone(),
        &format!("/api/chatui_project/get_sessions?project_id={project_id}"),
    )
    .await;
    let empty_sessions: serde_json::Value = response_json(empty_sessions_response).await;
    assert_eq!(
        empty_sessions["data"]
            .as_array()
            .expect("empty sessions")
            .len(),
        0
    );

    let delete_response = get(
        router.clone(),
        &format!("/api/chatui_project/delete?project_id={project_id}"),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let missing_response = get(
        router,
        &format!("/api/chatui_project/get?project_id={project_id}"),
    )
    .await;
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn management_session_rule_routes_delegate_to_typed_repositories() {
    let repository = Arc::new(InMemorySessionRuleRepository::new());
    let state = management_state_fixture().with_session_rules(ManagementSessionRuleState::new(
        repository.clone(),
        repository,
    ));
    let router = management_router(state);

    let update_response = post_json(
        router.clone(),
        "/api/management/session-rules/update",
        json!({
            "umo": "webchat:group:room-1",
            "key": { "type": "service" },
            "value": {
                "kind": "service",
                "session_enabled": true,
                "llm_enabled": false,
                "tts_enabled": true,
                "custom_name": "Room One"
            }
        }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);

    let provider_batch = post_json(
        router.clone(),
        "/api/management/session-rules/batch-provider",
        json!({
            "scope": "all",
            "all_umos": ["webchat:group:room-1", "webchat:private:user-1"],
            "capability": ProviderCapability::ChatCompletion,
            "provider_id": "provider-a"
        }),
    )
    .await;
    assert_eq!(provider_batch.status(), StatusCode::OK);
    let batch_payload: serde_json::Value = response_json(provider_batch).await;
    assert_eq!(batch_payload["success_count"], 2);

    let create_group = post_json(
        router.clone(),
        "/api/management/session-rules/groups/upsert",
        json!({
            "id": "team",
            "name": "Team",
            "umos": ["webchat:group:room-1"]
        }),
    )
    .await;
    assert_eq!(create_group.status(), StatusCode::OK);

    let service_batch = post_json(
        router.clone(),
        "/api/management/session-rules/batch-service",
        json!({
            "scope": { "custom_group": "team" },
            "all_umos": [],
            "patch": { "session_enabled": false }
        }),
    )
    .await;
    assert_eq!(service_batch.status(), StatusCode::OK);

    let list_response = get(router.clone(), "/api/management/session-rules").await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert_eq!(list["rules"].as_array().expect("rules").len(), 2);
    assert!(
        list["available_rule_keys"]
            .as_array()
            .expect("keys")
            .iter()
            .any(|key| key == &json!({ "type": "provider", "capability": "chat_completion" }))
    );

    let groups_response = get(router, "/api/management/session-rules/groups").await;
    assert_eq!(groups_response.status(), StatusCode::OK);
    let groups: serde_json::Value = response_json(groups_response).await;
    assert_eq!(groups["groups"][0]["id"], "team");
    assert_eq!(groups["groups"][0]["umo_count"], 1);
}

#[tokio::test]
async fn source_session_facade_matches_dashboard_rule_and_group_shapes() {
    let repository = Arc::new(InMemorySessionRuleRepository::new());
    let conversations = ConversationService::new();
    conversations
        .upsert(
            ConversationRecord::new("webchat", "room-1")
                .with_user_id("webchat:GroupMessage:room-1")
                .with_title("Room One"),
        )
        .await
        .expect("conversation should seed active umo");
    conversations
        .upsert(
            ConversationRecord::new("webchat", "user-1")
                .with_user_id("webchat:FriendMessage:user-1")
                .with_title("User One"),
        )
        .await
        .expect("conversation should seed active umo");
    let state = management_state_fixture()
        .with_session_rules(ManagementSessionRuleState::new(
            repository.clone(),
            repository,
        ))
        .with_conversations(ManagementConversationState::new(conversations));
    let router = management_router(state);

    let update_response = post_json(
        router.clone(),
        "/api/session/update-rule",
        json!({
            "umo": "webchat:GroupMessage:room-1",
            "rule_key": "session_service_config",
            "rule_value": {
                "session_enabled": true,
                "llm_enabled": false,
                "tts_enabled": true,
                "custom_name": "Ops Room",
                "persona_id": "default"
            }
        }),
    )
    .await;
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated: serde_json::Value = response_json(update_response).await;
    assert_eq!(updated["status"], "ok");

    let provider_response = post_json(
        router.clone(),
        "/api/session/update-rule",
        json!({
            "umo": "webchat:GroupMessage:room-1",
            "rule_key": "provider_perf_chat_completion",
            "rule_value": "provider-a"
        }),
    )
    .await;
    assert_eq!(provider_response.status(), StatusCode::OK);

    let list_response = get(
        router.clone(),
        "/api/session/list-rule?page=1&page_size=10&search=ops",
    )
    .await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert_eq!(list["status"], "ok");
    assert_eq!(list["data"]["total"], 1);
    assert_eq!(list["data"]["rules"][0]["platform"], "webchat");
    assert_eq!(
        list["data"]["rules"][0]["rules"]["session_service_config"]["custom_name"],
        "Ops Room"
    );
    assert_eq!(
        list["data"]["rules"][0]["rules"]["provider_perf_chat_completion"],
        "provider-a"
    );
    assert!(
        list["data"]["available_rule_keys"]
            .as_array()
            .expect("available keys")
            .contains(&json!("session_plugin_config"))
    );

    let active_response = get(router.clone(), "/api/session/active-umos").await;
    assert_eq!(active_response.status(), StatusCode::OK);
    let active: serde_json::Value = response_json(active_response).await;
    assert!(
        active["data"]["umos"]
            .as_array()
            .expect("umos")
            .contains(&json!("webchat:FriendMessage:user-1"))
    );

    let group_response = post_json(
        router.clone(),
        "/api/session/group/create",
        json!({
            "name": "Ops",
            "umos": ["webchat:GroupMessage:room-1"]
        }),
    )
    .await;
    assert_eq!(group_response.status(), StatusCode::OK);
    let group_payload: serde_json::Value = response_json(group_response).await;
    let group_id = group_payload["data"]["group"]["id"]
        .as_str()
        .expect("group id")
        .to_string();

    let add_response = post_json(
        router.clone(),
        "/api/session/group/update",
        json!({
            "id": group_id,
            "add_umos": ["webchat:FriendMessage:user-1"]
        }),
    )
    .await;
    assert_eq!(add_response.status(), StatusCode::OK);

    let batch_service = post_json(
        router.clone(),
        "/api/session/batch-update-service",
        json!({
            "scope": "custom_group",
            "group_id": group_id,
            "tts_enabled": false
        }),
    )
    .await;
    assert_eq!(batch_service.status(), StatusCode::OK);
    let batch_payload: serde_json::Value = response_json(batch_service).await;
    assert_eq!(batch_payload["data"]["success_count"], 2);

    let status_response = get(
        router.clone(),
        "/api/session/list-all-with-status?message_type=group&search=ops",
    )
    .await;
    assert_eq!(status_response.status(), StatusCode::OK);
    let status_payload: serde_json::Value = response_json(status_response).await;
    assert_eq!(
        status_payload["data"]["sessions"][0]["custom_name"],
        "Ops Room"
    );
    assert_eq!(status_payload["data"]["sessions"][0]["tts_enabled"], false);

    let batch_delete = post_json(
        router,
        "/api/session/batch-delete-rule",
        json!({ "umos": ["webchat:GroupMessage:room-1", "webchat:FriendMessage:user-1"] }),
    )
    .await;
    assert_eq!(batch_delete.status(), StatusCode::OK);
    let delete_payload: serde_json::Value = response_json(batch_delete).await;
    assert_eq!(delete_payload["data"]["deleted_count"], 2);
}

#[tokio::test]
async fn management_file_download_route_consumes_scoped_file_token() {
    let path = temp_management_file_path("download.txt");
    let _ = fs::remove_file(&path);
    fs::write(&path, "download body").expect("download fixture should write");
    let repository = Arc::new(InMemoryFileTokenRepository::new());
    repository
        .put_file_token(
            FileTokenRecord::new("token-1", &path, FileTokenScope::Dashboard)
                .with_filename("download.txt")
                .with_content_type("text/plain"),
        )
        .await
        .expect("file token should store");
    let state = management_state_fixture()
        .with_file_downloads(ManagementFileDownloadState::new(repository));
    let router = management_router(state);

    let response = get(router.clone(), "/api/management/files/token-1").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("text/plain")
    );
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("download body should read");
    assert_eq!(&body[..], b"download body");

    let second_response = get(router, "/api/management/files/token-1").await;
    assert_eq!(second_response.status(), StatusCode::NOT_FOUND);

    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn management_file_upload_route_writes_sqlite_attachment_token_and_downloads_stream() {
    let root = temp_management_dir_path("file-upload-sqlite");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("upload root should be created");
    let db_path = root.join("main.db");
    let router = management_router(
        management_state_fixture()
            .with_sqlite_storage_path(&db_path)
            .expect("sqlite file storage should build"),
    );
    let boundary = "astrbot-upload-boundary";
    let body = multipart_body(
        boundary,
        &[
            ("attachment_id", "att-upload"),
            ("scope", "openapi.file"),
            ("single_use", "false"),
            ("expires_at_unix", "5000000000"),
        ],
        "file",
        "../note.txt",
        "text/plain",
        b"streamed upload body",
    );

    let response = post_multipart(
        router.clone(),
        "/api/management/files/upload",
        boundary,
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response_json(response).await;
    assert_eq!(payload["attachment_id"], "att-upload");
    assert_eq!(payload["filename"], "note.txt");
    assert_eq!(payload["content_type"], "text/plain");
    assert_eq!(payload["scope"], "openapi.file");
    assert_eq!(payload["single_use"], false);
    assert_eq!(payload["size_bytes"], 20);

    let token = payload["token"].as_str().expect("token should be present");
    let reloaded = SqliteStorage::open(&db_path).expect("sqlite should reopen");
    let attachment = reloaded
        .attachment("att-upload")
        .await
        .expect("attachment should load")
        .expect("attachment should exist");
    assert_eq!(attachment.filename.as_deref(), Some("note.txt"));
    assert_eq!(
        attachment.stored_url.as_deref(),
        Some(payload["download_url"].as_str().expect("download url"))
    );
    let token_record = reloaded
        .file_token(token)
        .await
        .expect("file token should load")
        .expect("file token should exist");
    assert_eq!(token_record.scope, FileTokenScope::OpenApiFile);
    assert!(!token_record.single_use);
    let attachment_root =
        fs::canonicalize(root.join("attachments")).expect("attachment root should exist");
    assert!(token_record.file_path.starts_with(&attachment_root));
    assert_eq!(
        fs::read(&token_record.file_path).expect("uploaded file should read"),
        b"streamed upload body"
    );

    let file_response = get(router.clone(), &format!("/api/management/files/{token}")).await;
    assert_eq!(file_response.status(), StatusCode::OK);
    let body = to_bytes(file_response.into_body(), usize::MAX)
        .await
        .expect("download body should read");
    assert_eq!(&body[..], b"streamed upload body");

    let second_response = get(router.clone(), &format!("/api/management/files/{token}")).await;
    assert_eq!(second_response.status(), StatusCode::OK);

    drop(router);
    drop(reloaded);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn management_file_download_route_rejects_expired_missing_and_traversal_tokens() {
    let root = temp_management_dir_path("file-download-security");
    let outside = temp_management_file_path("file-download-outside.txt");
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_file(&outside);
    fs::create_dir_all(&root).expect("download root should be created");
    let valid = root.join("valid.txt");
    fs::write(&valid, b"valid").expect("valid fixture should write");
    fs::write(&outside, b"outside").expect("outside fixture should write");
    let repository = Arc::new(InMemoryFileTokenRepository::new());
    repository
        .put_file_token(
            FileTokenRecord::new("expired", &valid, FileTokenScope::Attachment).expires_at_unix(1),
        )
        .await
        .expect("expired token should store");
    repository
        .put_file_token(FileTokenRecord::new(
            "missing",
            root.join("missing.txt"),
            FileTokenScope::Attachment,
        ))
        .await
        .expect("missing token should store");
    repository
        .put_file_token(FileTokenRecord::new(
            "outside",
            &outside,
            FileTokenScope::Attachment,
        ))
        .await
        .expect("outside token should store");
    let state = management_state_fixture().with_file_downloads(
        ManagementFileDownloadState::new(repository).with_allowed_root(root.clone()),
    );
    let router = management_router(state);

    let expired_response = get(router.clone(), "/api/management/files/expired").await;
    assert_eq!(expired_response.status(), StatusCode::NOT_FOUND);

    let missing_response = get(router.clone(), "/api/management/files/missing").await;
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);

    let outside_response = get(router, "/api/management/files/outside").await;
    assert_eq!(outside_response.status(), StatusCode::FORBIDDEN);

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(outside);
}

#[tokio::test]
async fn management_file_download_route_streams_large_reusable_openapi_file() {
    let root = temp_management_dir_path("file-download-large");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("download root should be created");
    let path = root.join("large.bin");
    let data = (0..(1024 * 1024 + 17))
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    fs::write(&path, &data).expect("large fixture should write");
    let repository = Arc::new(InMemoryFileTokenRepository::new());
    repository
        .put_file_token(
            FileTokenRecord::new("openapi-large", &path, FileTokenScope::OpenApiFile)
                .with_filename("large.bin")
                .reusable(),
        )
        .await
        .expect("openapi file token should store");
    let state = management_state_fixture().with_file_downloads(
        ManagementFileDownloadState::new(repository).with_allowed_root(root.clone()),
    );
    let router = management_router(state);

    for _ in 0..2 {
        let response = get(router.clone(), "/api/management/files/openapi-large").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("large body should stream");
        assert_eq!(&body[..], &data[..]);
    }

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn management_backup_precheck_route_delegates_to_backup_state() {
    let manifest = BackupManifest::new("4.9.1", "2026-05-16T00:00:00Z")
        .with_table_group("main_db", ["conversations"]);
    let state = management_state_fixture().with_backup(backup_management_state("4.9.2"));
    let router = management_router(state);

    let response = post_json(
        router,
        "/api/management/backup/precheck",
        json!({ "manifest": manifest }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response_json(response).await;
    assert_eq!(payload["precheck"]["valid"], true);
    assert_eq!(payload["precheck"]["can_import"], true);
    assert_eq!(payload["precheck"]["version_status"], "MinorDiff");
    assert_eq!(payload["precheck"]["backup_summary"]["table_groups"], 1);
}

#[tokio::test]
async fn management_backup_job_routes_delegate_to_service_progress() {
    let root = temp_management_backup_chunk_path("job-routes");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("backup root should be created");
    fs::write(root.join("backup.zip"), b"backup-fixture").expect("backup fixture should write");
    let state =
        management_state_fixture().with_backup(backup_management_state_with_root("4.9.1", &root));
    let router = management_router(state);

    let export_response = post_json(
        router.clone(),
        "/api/management/backup/export",
        json!({
            "task_id": "export-route",
            "astrbot_version": "4.9.1",
            "exported_at": "2026-05-16T00:00:00Z"
        }),
    )
    .await;
    assert_eq!(export_response.status(), StatusCode::OK);
    let export_payload: serde_json::Value = response_json(export_response).await;
    assert_eq!(export_payload["task"]["progress"]["status"], "Completed");

    let progress_response = get(
        router.clone(),
        "/api/management/backup/progress/export-route",
    )
    .await;
    assert_eq!(progress_response.status(), StatusCode::OK);
    let progress_payload: serde_json::Value = response_json(progress_response).await;
    assert_eq!(progress_payload["task"]["task_id"], "export-route");
    assert_eq!(progress_payload["task"]["kind"], "Export");

    let progress_catalog_response = get(router.clone(), "/api/management/backup/progress").await;
    assert_eq!(progress_catalog_response.status(), StatusCode::OK);
    let progress_catalog: serde_json::Value = response_json(progress_catalog_response).await;
    assert_eq!(progress_catalog["tasks"][0]["task_id"], "export-route");

    let import_response = post_json(
        router,
        "/api/management/backup/import",
        json!({
            "task_id": "import-route",
            "source_id": "backup.zip",
            "mode": "Replace",
            "confirmed": true
        }),
    )
    .await;
    assert_eq!(import_response.status(), StatusCode::OK);
    let import_payload: serde_json::Value = response_json(import_response).await;
    assert_eq!(import_payload["task"]["kind"], "Import");
    assert_eq!(import_payload["task"]["progress"]["status"], "Completed");

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn management_backup_routes_execute_real_zip_export_precheck_and_restore() {
    let root = temp_management_dir_path("backup-real-routes");
    let _ = fs::remove_dir_all(&root);
    let backup_root = root.join("backups");
    let source_db = root.join("source/main.sqlite");
    let target_db = root.join("target/main.sqlite");
    let source_config = root.join("source/config");
    let target_config = root.join("target/config");
    fs::create_dir_all(&source_config).expect("source config dir should be created");
    fs::write(
        source_config.join("cmd_config.json"),
        br#"{"dashboard":true}"#,
    )
    .expect("source config should write");

    let source = SqliteStorage::open(&source_db).expect("source sqlite should open");
    source
        .store_api_key(ApiKeyRecord::new(
            "key-real",
            "Dashboard",
            "hash-real",
            "ak_real_",
            ["management.read"],
            "admin",
        ))
        .await
        .expect("source api key should persist");

    let backup_state = ManagementBackupState::with_roots(
        Arc::new(BackupJobService::new(
            Arc::new(
                SqliteBackupRepository::new(&source_db, &backup_root)
                    .with_directory("config", &source_config),
            ),
            Arc::new(FilesystemBackupExporter::new(&backup_root)),
            Arc::new(
                SqliteBackupImporter::new("4.9.1", &target_db)
                    .with_directory("config", &target_config),
            ),
        )),
        &backup_root,
        backup_root.join("chunks"),
    );
    let router = management_router(management_state_fixture().with_backup(backup_state));

    let export_response = post_json(
        router.clone(),
        "/api/management/backup/export",
        json!({
            "task_id": "export-real",
            "astrbot_version": "4.9.1",
            "exported_at": "2026-05-18T00:00:00Z"
        }),
    )
    .await;
    assert_eq!(export_response.status(), StatusCode::OK);
    let backup_file = backup_root.join("export-real.zip");
    let manifest = verify_backup_archive(&backup_file).expect("backup zip should verify");
    assert!(
        manifest
            .checksums
            .get("databases/main_db/api_keys.json")
            .is_some_and(|checksum| checksum.starts_with("sha256:"))
    );

    let precheck_response = post_json(
        router.clone(),
        "/api/management/backup/precheck",
        json!({ "filename": "export-real.zip" }),
    )
    .await;
    assert_eq!(precheck_response.status(), StatusCode::OK);
    let precheck_payload: serde_json::Value = response_json(precheck_response).await;
    assert_eq!(precheck_payload["precheck"]["valid"], true);
    assert_eq!(precheck_payload["precheck"]["can_import"], true);
    assert!(
        precheck_payload["precheck"]["backup_summary"]["checksums"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );

    let import_response = post_json(
        router,
        "/api/management/backup/import",
        json!({
            "task_id": "import-real",
            "source_id": "export-real.zip",
            "mode": "Replace",
            "confirmed": true
        }),
    )
    .await;
    assert_eq!(import_response.status(), StatusCode::OK);
    let import_payload: serde_json::Value = response_json(import_response).await;
    assert_eq!(import_payload["task"]["progress"]["status"], "Completed");

    let restored = SqliteStorage::open(&target_db).expect("target sqlite should open");
    assert!(
        restored
            .api_key_by_hash("hash-real")
            .await
            .expect("restored api key query")
            .is_some()
    );
    assert_eq!(
        fs::read_to_string(target_config.join("cmd_config.json")).expect("restored config"),
        r#"{"dashboard":true}"#
    );

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn management_backup_import_bad_zip_records_failed_progress() {
    let root = temp_management_dir_path("backup-bad-zip-route");
    let _ = fs::remove_dir_all(&root);
    let backup_root = root.join("backups");
    fs::create_dir_all(&backup_root).expect("backup root should be created");
    fs::write(backup_root.join("bad.zip"), b"not-a-zip").expect("bad zip should write");
    let backup_state = ManagementBackupState::with_roots(
        Arc::new(BackupJobService::new(
            Arc::new(SqliteBackupRepository::new(
                root.join("source.sqlite"),
                &backup_root,
            )),
            Arc::new(FilesystemBackupExporter::new(&backup_root)),
            Arc::new(SqliteBackupImporter::new(
                "4.9.1",
                root.join("target.sqlite"),
            )),
        )),
        &backup_root,
        backup_root.join("chunks"),
    );
    let router = management_router(management_state_fixture().with_backup(backup_state));

    let import_response = post_json(
        router.clone(),
        "/api/management/backup/import",
        json!({
            "task_id": "import-bad",
            "source_id": "bad.zip",
            "mode": "Replace",
            "confirmed": true
        }),
    )
    .await;
    assert_eq!(import_response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let progress_response = get(router, "/api/management/backup/progress/import-bad").await;
    assert_eq!(progress_response.status(), StatusCode::OK);
    let progress_payload: serde_json::Value = response_json(progress_response).await;
    assert_eq!(progress_payload["task"]["progress"]["status"], "Failed");

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn management_backup_upload_routes_delegate_to_upload_manager() {
    let root = temp_management_backup_chunk_path("upload-route");
    let _ = fs::remove_dir_all(&root);
    let state =
        management_state_fixture().with_backup(backup_management_state_with_root("4.9.1", &root));
    let router = management_router(state);

    let start_response = post_json(
        router.clone(),
        "/api/management/backup/upload/start",
        json!({
            "upload_id": "upload-route",
            "filename": "../backup.zip",
            "total_size": BACKUP_UPLOAD_CHUNK_SIZE + 10,
            "now_unix": 100
        }),
    )
    .await;
    assert_eq!(start_response.status(), StatusCode::OK);
    let start_payload: serde_json::Value = response_json(start_response).await;
    assert_eq!(start_payload["session"]["filename"], "backup.zip");
    assert_eq!(start_payload["session"]["total_chunks"], 2);

    for (chunk_index, bytes_len) in [(0, BACKUP_UPLOAD_CHUNK_SIZE), (1, 10)] {
        let chunk_response = post_json(
            router.clone(),
            "/api/management/backup/upload/chunk",
            json!({
                "upload_id": "upload-route",
                "chunk_index": chunk_index,
                "bytes_len": bytes_len,
                "now_unix": 110 + chunk_index
            }),
        )
        .await;
        assert_eq!(chunk_response.status(), StatusCode::OK);
    }

    let complete_response = post_json(
        router.clone(),
        "/api/management/backup/upload/complete",
        json!({ "upload_id": "upload-route" }),
    )
    .await;
    assert_eq!(complete_response.status(), StatusCode::OK);
    let complete_payload: serde_json::Value = response_json(complete_response).await;
    assert_eq!(complete_payload["plan"]["filename"], "backup.zip");
    assert_eq!(
        complete_payload["plan"]["ordered_chunk_indexes"],
        json!([0, 1])
    );
    assert!(root.join("backup.zip").is_file());
    assert!(!root.join("upload-route").exists());
    let completed_progress_response = get(
        router.clone(),
        "/api/management/backup/progress/upload-route",
    )
    .await;
    assert_eq!(completed_progress_response.status(), StatusCode::OK);
    let completed_progress: serde_json::Value = response_json(completed_progress_response).await;
    assert_eq!(
        completed_progress["task"]["progress"]["status"],
        "Completed"
    );

    let abort_start_response = post_json(
        router.clone(),
        "/api/management/backup/upload/start",
        json!({
            "upload_id": "upload-abort",
            "filename": "abort.zip",
            "total_size": 10,
            "now_unix": 200
        }),
    )
    .await;
    assert_eq!(abort_start_response.status(), StatusCode::OK);

    let abort_response = post_json(
        router.clone(),
        "/api/management/backup/upload/abort",
        json!({ "upload_id": "upload-abort" }),
    )
    .await;
    assert_eq!(abort_response.status(), StatusCode::OK);
    let abort_payload: serde_json::Value = response_json(abort_response).await;
    assert_eq!(abort_payload["aborted"], true);
    let cancelled_progress_response = get(
        router.clone(),
        "/api/management/backup/progress/upload-abort",
    )
    .await;
    assert_eq!(cancelled_progress_response.status(), StatusCode::OK);
    let cancelled_progress: serde_json::Value = response_json(cancelled_progress_response).await;
    assert_eq!(
        cancelled_progress["task"]["progress"]["status"],
        "Cancelled"
    );

    let bytes_start_response = post_json(
        router.clone(),
        "/api/management/backup/upload/start",
        json!({
            "upload_id": "upload-bytes",
            "filename": "bytes.zip",
            "total_size": 5,
            "now_unix": 300
        }),
    )
    .await;
    assert_eq!(bytes_start_response.status(), StatusCode::OK);
    let bytes_chunk_response = post_json(
        router.clone(),
        "/api/management/backup/upload/chunk",
        json!({
            "upload_id": "upload-bytes",
            "chunk_index": 0,
            "bytes_len": 5,
            "bytes_base64": "aGVsbG8=",
            "now_unix": 301
        }),
    )
    .await;
    assert_eq!(bytes_chunk_response.status(), StatusCode::OK);
    let bytes_complete_response = post_json(
        router,
        "/api/management/backup/upload/complete",
        json!({ "upload_id": "upload-bytes" }),
    )
    .await;
    assert_eq!(bytes_complete_response.status(), StatusCode::OK);
    assert_eq!(
        fs::read(root.join("bytes.zip")).expect("merged bytes upload"),
        b"hello"
    );

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn management_backup_file_routes_manage_safe_backup_files() {
    let root = temp_management_backup_chunk_path("file-routes");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("backup root should be created");
    fs::write(root.join("report.zip"), b"backup-bytes").expect("backup fixture should write");

    let tokens: Arc<dyn FileTokenRepository> = Arc::new(InMemoryFileTokenRepository::new());
    let state = management_state_fixture()
        .with_backup(backup_management_state_with_root("4.9.1", &root))
        .with_file_downloads(ManagementFileDownloadState::new(tokens));
    let router = management_router(state);

    let catalog_response = get(router.clone(), "/api/management/backup/files").await;
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response_json(catalog_response).await;
    assert_eq!(catalog["files"][0]["filename"], "report.zip");
    assert_eq!(catalog["files"][0]["size_bytes"], 12);

    let download_response = post_json(
        router.clone(),
        "/api/management/backup/files/download",
        json!({ "filename": "report.zip" }),
    )
    .await;
    assert_eq!(download_response.status(), StatusCode::OK);
    let download: serde_json::Value = response_json(download_response).await;
    let token = download["token"].as_str().expect("token should be present");
    let file_response = get(router.clone(), &format!("/api/management/files/{token}")).await;
    assert_eq!(file_response.status(), StatusCode::OK);
    let body = to_bytes(file_response.into_body(), usize::MAX)
        .await
        .expect("download body should read");
    assert_eq!(&body[..], b"backup-bytes");

    let rename_response = post_json(
        router.clone(),
        "/api/management/backup/files/rename",
        json!({ "filename": "report.zip", "new_filename": "renamed.zip" }),
    )
    .await;
    assert_eq!(rename_response.status(), StatusCode::OK);
    assert!(root.join("renamed.zip").is_file());

    let restore_response = post_json(
        router.clone(),
        "/api/management/backup/files/restore",
        json!({
            "filename": "renamed.zip",
            "task_id": "restore-file-route",
            "mode": "Merge",
            "confirmed": true
        }),
    )
    .await;
    assert_eq!(restore_response.status(), StatusCode::OK);
    let restore: serde_json::Value = response_json(restore_response).await;
    assert_eq!(restore["task"]["kind"], "Import");

    let delete_response = post_json(
        router.clone(),
        "/api/management/backup/files/delete",
        json!({ "filename": "renamed.zip" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted: serde_json::Value = response_json(delete_response).await;
    assert_eq!(deleted["deleted"], true);
    assert!(!root.join("renamed.zip").exists());

    let unsafe_response = post_json(
        router,
        "/api/management/backup/files/download",
        json!({ "filename": "../report.zip" }),
    )
    .await;
    assert_eq!(unsafe_response.status(), StatusCode::BAD_REQUEST);

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn source_compatible_backup_facades_wrap_files_tasks_and_downloads() {
    let root = temp_management_backup_chunk_path("source-backup-routes");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("backup root should be created");
    fs::write(root.join("report.zip"), b"backup-bytes").expect("backup fixture should write");

    let state =
        management_state_fixture().with_backup(backup_management_state_with_root("4.9.1", &root));
    let router = management_router(state);

    let list_response = get(router.clone(), "/api/backup/list?page=1&page_size=10").await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list: serde_json::Value = response_json(list_response).await;
    assert_eq!(list["status"], "ok");
    assert_eq!(list["data"]["items"][0]["filename"], "report.zip");
    assert_eq!(list["data"]["items"][0]["size"], 12);

    let export_response = post_json(router.clone(), "/api/backup/export", json!({})).await;
    assert_eq!(export_response.status(), StatusCode::OK);
    let export: serde_json::Value = response_json(export_response).await;
    assert_eq!(export["data"]["type"], "export");
    assert_eq!(export["data"]["status"], "completed");
    let task_id = export["data"]["task_id"].as_str().expect("task id");

    let progress_response = get(
        router.clone(),
        &format!("/api/backup/progress?task_id={task_id}"),
    )
    .await;
    assert_eq!(progress_response.status(), StatusCode::OK);
    let progress: serde_json::Value = response_json(progress_response).await;
    assert_eq!(progress["data"]["task_id"], task_id);

    let check_response = post_json(
        router.clone(),
        "/api/backup/check",
        json!({ "filename": "report.zip" }),
    )
    .await;
    assert_eq!(check_response.status(), StatusCode::OK);
    let check: serde_json::Value = response_json(check_response).await;
    assert_eq!(check["data"]["can_import"], true);

    let download_response = get(
        router.clone(),
        "/api/backup/download?filename=report.zip&token=test-token",
    )
    .await;
    assert_eq!(download_response.status(), StatusCode::OK);
    let body = to_bytes(download_response.into_body(), usize::MAX)
        .await
        .expect("source download body should read");
    assert_eq!(&body[..], b"backup-bytes");

    let rename_response = post_json(
        router.clone(),
        "/api/backup/rename",
        json!({ "filename": "report.zip", "new_name": "renamed" }),
    )
    .await;
    assert_eq!(rename_response.status(), StatusCode::OK);
    assert!(root.join("renamed.zip").is_file());

    let delete_response = post_json(
        router,
        "/api/backup/delete",
        json!({ "filename": "renamed.zip" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert!(!root.join("renamed.zip").exists());

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn management_backup_precheck_route_requires_configured_backup_state() {
    let router = management_router(management_state_fixture());

    let response = post_json(
        router,
        "/api/management/backup/precheck",
        json!({ "manifest": BackupManifest::new("4.9.1", "2026-05-16T00:00:00Z") }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

fn management_state_fixture() -> ManagementApiState {
    let provider_manager = ProviderManager::from_configs(
        &ProviderRegistry::with_builtin_providers(),
        ProviderManagerConfigSet {
            chat_providers: vec![ChatProviderConfig::mock("mock-provider", "ok")],
            default_chat_provider_id: Some("mock-provider".to_string()),
            embedding_providers: vec![EmbeddingProviderConfig::mock("embedding", vec![0.1, 0.9])],
            default_embedding_provider_id: Some("embedding".to_string()),
            ..ProviderManagerConfigSet::default()
        },
    )
    .expect("provider manager should build");

    let (event_tx, _event_rx) = mpsc::channel(1);
    let platform_manager = PlatformManager::from_configs(
        &PlatformRegistry::with_builtin_platforms(),
        vec![
            PlatformConfig::mock("mock-platform"),
            PlatformConfig::webchat("webchat"),
        ],
        PlatformBuildContext::new(event_tx),
    )
    .expect("platform manager should build");

    let mut plugin_registry = PluginRegistry::new();
    plugin_registry.register_handler(RegisteredHandler::new(
        HandlerMetadata::new("builtin", "ping", PluginEventType::AdapterMessage).with_priority(10),
        Arc::new(NoopPluginHandler),
    ));

    ManagementApiState::from_managers(&provider_manager, &platform_manager, &plugin_registry)
}

fn legacy_sha1_hash(secret: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Default)]
struct RecordingConfigApplyExecutor {
    actions: Mutex<Vec<RuntimeConfigReloadAction>>,
}

impl RecordingConfigApplyExecutor {
    fn actions(&self) -> Vec<RuntimeConfigReloadAction> {
        self.actions.lock().expect("actions lock").clone()
    }
}

impl ManagementConfigApplyExecutor for RecordingConfigApplyExecutor {
    fn apply_config_change<'a>(
        &'a self,
        request: ManagementConfigApplyExecutionRequest,
    ) -> ManagementConfigApplyFuture<'a> {
        Box::pin(async move {
            self.actions
                .lock()
                .map_err(|error| error.to_string())?
                .push(request.plan.reload_action);
            Ok(ManagementConfigApplyExecution::accepted(
                request.plan.reload_action,
                "recorded config apply",
            ))
        })
    }
}

#[derive(Debug, Clone)]
struct StaticProviderHealthCheck {
    mode: &'static str,
}

impl StaticProviderHealthCheck {
    fn available() -> Self {
        Self { mode: "available" }
    }

    fn timeout() -> Self {
        Self { mode: "timeout" }
    }

    fn credential() -> Self {
        Self { mode: "credential" }
    }
}

impl ManagementProviderHealthCheck for StaticProviderHealthCheck {
    fn check_provider<'a>(
        &'a self,
        provider: RuntimeChatProviderConfig,
    ) -> ManagementProviderHealthFuture<'a> {
        Box::pin(async move {
            Ok(match self.mode {
                "timeout" => ManagementProviderHealthResult::unavailable(
                    provider.id,
                    "timeout",
                    "provider health check timed out after 1s",
                    1000,
                ),
                "credential" => ManagementProviderHealthResult::unavailable(
                    provider.id,
                    "credential",
                    "provider returned 401 Unauthorized",
                    12,
                ),
                _ => ManagementProviderHealthResult::available(
                    provider.id,
                    "provider runtime responded to a lightweight chat request",
                    3,
                ),
            })
        })
    }

    fn discover_models<'a>(
        &'a self,
        source: Option<RuntimeProviderSourceConfig>,
        provider_type: String,
    ) -> ManagementProviderModelsFuture<'a> {
        Box::pin(async move {
            Ok(ManagementProviderModelsResult {
                provider_type,
                models: vec!["gpt-4.1-mini".to_string(), "gpt-4.1".to_string()],
                model_candidates: vec![
                    astrbot_provider::ProviderModelInfo::new("gpt-4.1-mini"),
                    astrbot_provider::ProviderModelInfo::new("gpt-4.1"),
                ],
                model_metadata: serde_json::Map::new(),
                dynamic: source.is_some(),
                unsupported: false,
                capability: astrbot_provider::ProviderCapability::ChatCompletion.to_string(),
                model_discovery: astrbot_provider::ProviderModelDiscoverySupport::Supported,
                source_id: source.map(|source| source.id),
                source: Some("test-checker".to_string()),
                error_kind: None,
                message: Some("test model discovery".to_string()),
            })
        })
    }
}

#[derive(Debug, Clone)]
struct StaticPlatformHealthCheck {
    mode: &'static str,
}

impl StaticPlatformHealthCheck {
    fn available() -> Self {
        Self { mode: "available" }
    }

    fn timeout() -> Self {
        Self { mode: "timeout" }
    }

    fn credential() -> Self {
        Self { mode: "credential" }
    }
}

impl ManagementPlatformHealthCheck for StaticPlatformHealthCheck {
    fn check_platform<'a>(
        &'a self,
        platform: RuntimePlatformConfig,
    ) -> ManagementPlatformHealthFuture<'a> {
        Box::pin(async move {
            let webhook_reachable = platform
                .options
                .get("webhook_uuid")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| !value.is_empty());
            Ok(match self.mode {
                "timeout" => ManagementPlatformHealthResult::unavailable(
                    platform.id,
                    "timeout",
                    "platform startup probe timed out",
                    1000,
                    webhook_reachable,
                ),
                "credential" => ManagementPlatformHealthResult::unavailable(
                    platform.id,
                    "credential",
                    "platform requires secret bot_token",
                    4,
                    webhook_reachable,
                ),
                _ => ManagementPlatformHealthResult::available(
                    platform.id,
                    "platform adapter started successfully",
                    2,
                    webhook_reachable,
                ),
            })
        })
    }
}

fn skill_management_state_fixture() -> ManagementSkillState {
    let catalog = SkillCatalog::from_skills([
        SkillDescriptor::new("local_writer", "C:/skills/local_writer/SKILL.md")
            .with_description("Local writer"),
        SkillDescriptor::new("synced", "C:/skills/synced/SKILL.md")
            .with_description("Local synced")
            .with_source(SkillSource::Local),
    ]);
    let sandbox_cache = SkillSandboxCache::from_entries([
        SkillSandboxEntry::new("preset").with_description("Sandbox preset"),
        SkillSandboxEntry::new("synced").with_description("Synced preset"),
    ]);

    ManagementSkillState::new(catalog).with_sandbox_cache(sandbox_cache, true)
}

fn plugin_lifecycle_state_fixture() -> ManagementPluginLifecycleState {
    plugin_lifecycle_state_with_root("plugins/weather")
}

fn plugin_lifecycle_state_with_root(
    root: impl Into<std::path::PathBuf>,
) -> ManagementPluginLifecycleState {
    ManagementPluginLifecycleState::new(vec![ManagementPluginSeed::new(
        PluginLoadSource::python_compat("weather").with_root_dir(root),
        PluginManifest::new("Weather", "0.1.0").with_description("Weather plugin"),
        PluginLifecycleState::Active,
    )])
}

fn tool_management_state_fixture() -> ManagementToolState {
    let mut catalog = ToolCatalog::new();
    catalog.add_tool(
        ToolDescriptor::new("weather")
            .with_description("Weather lookup")
            .with_source_metadata(ToolSourceMetadata::plugin("weather", "Weather Plugin")),
    );
    catalog.add_tool(
        ToolDescriptor::new("astr_kb_search")
            .with_description("Knowledge base search")
            .with_source(ToolSource::Internal),
    );

    ManagementToolState::new(catalog)
}

fn knowledge_base_management_state_fixture() -> ManagementKnowledgeBaseState {
    let provider_manager = ProviderManager::from_configs(
        &ProviderRegistry::with_builtin_providers(),
        ProviderManagerConfigSet {
            embedding_providers: vec![EmbeddingProviderConfig::mock("embedding", vec![0.1, 0.9])],
            default_embedding_provider_id: Some("embedding".to_string()),
            rerank_providers: vec![RerankProviderConfig::mock("rerank", vec![0.8])],
            default_rerank_provider_id: Some("rerank".to_string()),
            ..ProviderManagerConfigSet::default()
        },
    )
    .expect("knowledge base provider manager should build");

    ManagementKnowledgeBaseState::in_memory(provider_manager)
}

async fn seeded_knowledge_base_management_state_fixture() -> ManagementKnowledgeBaseState {
    let provider_manager = ProviderManager::from_configs(
        &ProviderRegistry::with_builtin_providers(),
        ProviderManagerConfigSet {
            embedding_providers: vec![EmbeddingProviderConfig::mock("embedding", vec![0.1, 0.9])],
            default_embedding_provider_id: Some("embedding".to_string()),
            rerank_providers: vec![RerankProviderConfig::mock("rerank", vec![0.8])],
            default_rerank_provider_id: Some("rerank".to_string()),
            ..ProviderManagerConfigSet::default()
        },
    )
    .expect("knowledge base provider manager should build");
    let management =
        KnowledgeBaseManagementService::new(Arc::new(InMemoryKnowledgeBaseManagementStore::new()));
    let vector_store = Arc::new(InMemoryVectorStore::default());
    let kb_id = KnowledgeBaseId::new("kb-1").expect("kb id");
    management
        .create_kb(KnowledgeBaseCreateCommand::new(
            kb_id.clone(),
            "Docs",
            "embedding",
        ))
        .await
        .expect("knowledge base should save");
    let doc_id = DocumentId::new("doc-1").expect("doc id");
    let mut document = KnowledgeDocument::new(doc_id.clone(), kb_id.clone(), "Intro", "text/plain");
    document.chunk_count = 2;
    management
        .upsert_document(document)
        .await
        .expect("document should save");
    let dashboard_chunk = KnowledgeChunk::new(
        ChunkId::new("chunk-dashboard").expect("chunk id"),
        kb_id.clone(),
        doc_id.clone(),
        0,
        "AstrBot dashboard search supports sparse knowledge retrieval.",
    )
    .with_metadata("doc_name", json!("Intro"));
    management
        .upsert_chunk(dashboard_chunk.clone())
        .await
        .expect("chunk should save");
    let other_chunk = KnowledgeChunk::new(
        ChunkId::new("chunk-other").expect("chunk id"),
        kb_id,
        doc_id,
        1,
        "Provider settings are configured separately.",
    )
    .with_metadata("doc_name", json!("Intro"));
    management
        .upsert_chunk(other_chunk.clone())
        .await
        .expect("chunk should save");
    vector_store
        .upsert_chunks(vec![
            EmbeddedKnowledgeChunk::new(dashboard_chunk, vec![0.1, 0.9]),
            EmbeddedKnowledgeChunk::new(other_chunk, vec![0.9, 0.1]),
        ])
        .await
        .expect("vectors should save");

    let provider_manager = Arc::new(provider_manager);
    ManagementKnowledgeBaseState::from_components(
        management,
        KnowledgeProviderPreflightService::new(
            provider_manager.clone(),
            Some(provider_manager.clone()),
        ),
        KnowledgeUploadTaskService::new(Arc::new(InMemoryKnowledgeUploadTaskStore::new())),
        provider_manager.clone(),
        Some(provider_manager),
        vector_store,
        Arc::new(InMemoryKnowledgeDocumentRepository::new()),
        Arc::new(InMemoryKnowledgeMediaStore::new()),
    )
}

fn temp_management_config_path(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "astrbot-web-management-config-{}-{}.json",
        std::process::id(),
        suffix
    ))
}

async fn serve_once_http_response(status: &str, content_type: &str, body: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test http listener should bind");
    let address = listener
        .local_addr()
        .expect("test http listener should have address");
    let status = status.to_string();
    let content_type = content_type.to_string();
    let body = body.to_string();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("test http accept");
        let mut buffer = [0_u8; 4096];
        let _ = stream.read(&mut buffer).await.expect("test http read");
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("test http write");
    });
    format!("http://{address}/v1")
}

fn temp_management_file_path(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "astrbot-web-management-file-{}-{}",
        std::process::id(),
        suffix
    ))
}

fn temp_management_dir_path(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "astrbot-web-management-dir-{}-{}",
        std::process::id(),
        suffix
    ))
}

fn multipart_body(
    boundary: &str,
    fields: &[(&str, &str)],
    file_field: &str,
    filename: &str,
    content_type: &str,
    file_bytes: &[u8],
) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"{file_field}\"; filename=\"{filename}\"\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(file_bytes);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

fn cleanup_sqlite_files(path: &std::path::Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(path.with_extension("db-wal"));
    let _ = fs::remove_file(path.with_extension("db-shm"));
    let _ = fs::remove_file(format!("{}-wal", path.display()));
    let _ = fs::remove_file(format!("{}-shm", path.display()));
}

fn temp_management_backup_chunk_path(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "astrbot-web-management-backup-chunks-{}-{}",
        std::process::id(),
        suffix
    ))
}

fn backup_management_state(current_version: &str) -> ManagementBackupState {
    backup_management_state_with_root(
        current_version,
        temp_management_backup_chunk_path(current_version),
    )
}

fn backup_management_state_with_root(
    current_version: &str,
    root: impl Into<std::path::PathBuf>,
) -> ManagementBackupState {
    ManagementBackupState::new(
        Arc::new(BackupJobService::new(
            Arc::new(ManagementBackupRepository {
                manifest: BackupManifest::new("4.9.1", "2026-05-16T00:00:00Z")
                    .with_table_group("main_db", ["conversations"]),
            }),
            Arc::new(ManagementBackupExporter),
            Arc::new(PrecheckBackupImportPort::new(current_version)),
        )),
        root,
    )
}

struct ManagementBackupRepository {
    manifest: BackupManifest,
}

#[async_trait]
impl BackupRepositoryPort for ManagementBackupRepository {
    async fn collect_export(
        &self,
        request: &BackupExportJobRequest,
    ) -> Result<BackupExportRequest> {
        Ok(
            BackupExportRequest::new(&request.astrbot_version, &request.exported_at)
                .with_table_dump(BackupTableDump::new(
                    "main_db",
                    "conversations",
                    vec![json!({ "task_id": request.task_id })],
                )),
        )
    }

    async fn load_import_manifest(
        &self,
        _request: &BackupImportJobRequest,
    ) -> Result<BackupManifest> {
        Ok(self.manifest.clone())
    }
}

struct ManagementBackupExporter;

#[async_trait]
impl BackupExportPort for ManagementBackupExporter {
    async fn export_backup(&self, request: BackupExportRequest) -> Result<BackupExportPackage> {
        Ok(BackupExportPackage::from_request(request))
    }
}

#[derive(Debug)]
struct EchoSubagentBridge;

impl ManagementSubagentExecutionBridge for EchoSubagentBridge {
    fn execute(
        &self,
        subagent: &ResolvedSubagent,
        request: &ManagementSubagentExecuteRequest,
    ) -> std::result::Result<ManagementSubagentExecutionResult, String> {
        Ok(ManagementSubagentExecutionResult {
            status: "completed".to_string(),
            output: format!("{}: {}", subagent.name, request.input),
        })
    }
}

#[derive(Debug)]
struct EchoRestartExecutor;

impl MaintenanceRestartExecutor for EchoRestartExecutor {
    fn restart(&self, request: &MaintenanceRestartRequest) -> std::result::Result<String, String> {
        Ok(format!(
            "restart accepted: {}",
            request.reason.as_deref().unwrap_or("manual")
        ))
    }
}

#[derive(Debug)]
struct RecordingMaintenanceExecutor {
    result: std::result::Result<Vec<String>, String>,
    calls: Mutex<Vec<String>>,
}

impl RecordingMaintenanceExecutor {
    fn success() -> Self {
        Self {
            result: Ok(vec!["executor completed external io".to_string()]),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn failure(error: impl Into<String>) -> Self {
        Self {
            result: Err(error.into()),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn record(&self, call: impl Into<String>) -> std::result::Result<Vec<String>, String> {
        self.calls.lock().expect("calls lock").push(call.into());
        self.result.clone()
    }
}

impl MaintenanceReleaseExecutor for RecordingMaintenanceExecutor {
    fn execute_project_update(
        &self,
        plan: ProjectUpdatePlan,
    ) -> crate::MaintenanceExecutionFuture<'_> {
        Box::pin(async move {
            self.record(format!(
                "project:{}",
                plan.version.as_deref().unwrap_or("latest")
            ))
        })
    }

    fn execute_dashboard_update(
        &self,
        plan: DashboardUpdatePlan,
    ) -> crate::MaintenanceExecutionFuture<'_> {
        Box::pin(async move {
            self.record(format!(
                "dashboard:{}",
                if plan.latest {
                    "latest"
                } else {
                    plan.version.as_str()
                }
            ))
        })
    }
}

impl MaintenancePackageExecutor for RecordingMaintenanceExecutor {
    fn install_package(
        &self,
        plan: MaintenancePackageInstallPlan,
    ) -> crate::MaintenanceExecutionFuture<'_> {
        Box::pin(async move {
            self.record(format!(
                "package:{}",
                plan.request
                    .package
                    .as_deref()
                    .or(plan.request.requirements_path.as_deref())
                    .unwrap_or("requirements")
            ))
        })
    }
}

impl MaintenanceMigrationExecutor for RecordingMaintenanceExecutor {
    fn run_migration(
        &self,
        request: MaintenanceMigrationRequest,
    ) -> crate::MaintenanceExecutionFuture<'_> {
        Box::pin(async move {
            self.record(format!(
                "migration:{}",
                request
                    .platform_id_map
                    .keys()
                    .next()
                    .cloned()
                    .unwrap_or_default()
            ))
        })
    }
}

struct NoopPluginHandler;

#[async_trait]
impl PluginHandler for NoopPluginHandler {
    async fn handle(&self, event: &mut MessageEvent) -> Result<PluginControl> {
        event.set_result(astrbot_core::MessageEventResult::general(
            MessageChain::plain("ok"),
        ));
        Ok(PluginControl::Continue)
    }
}

struct ManagementNoopMessageSink;

#[async_trait]
impl astrbot_core::MessageSink for ManagementNoopMessageSink {
    async fn send(
        &self,
        _session: &astrbot_core::MessageSession,
        _chain: MessageChain,
    ) -> Result<()> {
        Ok(())
    }
}

struct PrecheckBackupImportPort {
    current_version: String,
}

impl PrecheckBackupImportPort {
    fn new(current_version: impl Into<String>) -> Self {
        Self {
            current_version: current_version.into(),
        }
    }
}

#[async_trait]
impl BackupImportPort for PrecheckBackupImportPort {
    async fn precheck_backup(&self, manifest: &BackupManifest) -> Result<BackupImportPrecheck> {
        Ok(BackupImportPrecheck::from_manifest(
            manifest,
            self.current_version.clone(),
        ))
    }

    async fn import_backup(
        &self,
        _manifest: BackupManifest,
        _mode: BackupImportMode,
    ) -> Result<BackupImportResult> {
        Ok(BackupImportResult::success())
    }
}
