use std::{fs, sync::Arc};

use astrbot_core::{MessageChain, MessageEvent, Result};
use astrbot_platform::{PlatformBuildContext, PlatformConfig, PlatformManager, PlatformRegistry};
use astrbot_plugin::{
    HandlerMetadata, PluginCompatibility, PluginControl, PluginEventType, PluginHandler,
    PluginInstallSource, PluginMarketEntry, PluginPackageDescriptor, PluginRegistry,
    RegisteredHandler,
};
use astrbot_provider::{
    ChatProviderConfig, ProviderManager, ProviderManagerConfigSet, ProviderRegistry,
};
use astrbot_runtime::{RuntimeConfig, RuntimeConfigService};
use astrbot_skill::{
    SkillCatalog, SkillDescriptor, SkillSandboxCache, SkillSandboxEntry, SkillSource,
};
use astrbot_storage::{
    BACKUP_UPLOAD_CHUNK_SIZE, BackupExportJobRequest, BackupExportPackage, BackupExportPort,
    BackupExportRequest, BackupImportJobRequest, BackupImportMode, BackupImportPort,
    BackupImportPrecheck, BackupImportResult, BackupJobService, BackupManifest,
    BackupRepositoryPort, BackupTableDump, FileTokenRecord, FileTokenRepository, FileTokenScope,
    InMemoryFileTokenRepository,
};
use astrbot_tool::{ToolCatalog, ToolDescriptor, ToolSource, ToolSourceMetadata};
use async_trait::async_trait;
use axum::{
    body::to_bytes,
    http::{StatusCode, header::CONTENT_TYPE},
};
use serde_json::json;
use tokio::sync::mpsc;

use crate::{
    DashboardAuthPolicy, ManagementApiState, ManagementAuthState, ManagementBackupState,
    ManagementFileDownloadState, ManagementSkillState, ManagementStatusResponse,
    ManagementToolState, PluginMarketManagementState, management_router,
    management_router_with_auth,
};

use super::support::{get, get_with_bearer, post_json, response_json};

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
async fn management_router_with_auth_requires_bearer_token() {
    let router = management_router_with_auth(
        management_state_fixture(),
        ManagementAuthState::new(DashboardAuthPolicy::new("secret")),
    );

    let unauthorized = get(router.clone(), "/api/management/status").await;
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let authorized = get_with_bearer(router, "/api/management/status", "secret").await;
    assert_eq!(authorized.status(), StatusCode::OK);
    let payload: ManagementStatusResponse = response_json(authorized).await;
    assert_eq!(payload.providers.chat_provider_count, 1);
}

#[tokio::test]
async fn management_config_routes_delegate_to_runtime_config_service() {
    let path = temp_management_config_path("apply");
    let _ = std::fs::remove_file(&path);
    let state = management_state_fixture().with_config_service(RuntimeConfigService::new(&path));
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
async fn management_skill_routes_expose_catalog_cache_and_side_effect_free_plans() {
    let state = management_state_fixture().with_skills(skill_management_state_fixture());
    let router = management_router(state);

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

    let delete_response = post_json(
        router,
        "/api/management/skills/delete-plan",
        json!({ "name": "local_writer" }),
    )
    .await;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete: serde_json::Value = response_json(delete_response).await;
    assert_eq!(delete["plan"]["skill_name"], "local_writer");
    assert_eq!(delete["plan"]["remove_local_dir"], true);
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

    let delete_response = post_json(
        router,
        "/api/management/skills/delete-plan",
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
    let state = management_state_fixture().with_backup(backup_management_state("4.9.1"));
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
}

#[tokio::test]
async fn management_backup_upload_routes_delegate_to_upload_manager() {
    let state = management_state_fixture().with_backup(backup_management_state("4.9.1"));
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
        router,
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

fn temp_management_config_path(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "astrbot-web-management-config-{}-{}.json",
        std::process::id(),
        suffix
    ))
}

fn temp_management_file_path(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "astrbot-web-management-file-{}-{}",
        std::process::id(),
        suffix
    ))
}

fn temp_management_backup_chunk_path(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "astrbot-web-management-backup-chunks-{}-{}",
        std::process::id(),
        suffix
    ))
}

fn backup_management_state(current_version: &str) -> ManagementBackupState {
    ManagementBackupState::new(
        Arc::new(BackupJobService::new(
            Arc::new(ManagementBackupRepository {
                manifest: BackupManifest::new("4.9.1", "2026-05-16T00:00:00Z")
                    .with_table_group("main_db", ["conversations"]),
            }),
            Arc::new(ManagementBackupExporter),
            Arc::new(PrecheckBackupImportPort::new(current_version)),
        )),
        temp_management_backup_chunk_path(current_version),
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
