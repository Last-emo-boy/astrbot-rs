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
use astrbot_storage::{
    FileTokenRecord, FileTokenRepository, FileTokenScope, InMemoryFileTokenRepository,
};
use async_trait::async_trait;
use axum::{
    body::to_bytes,
    http::{StatusCode, header::CONTENT_TYPE},
};
use serde_json::json;
use tokio::sync::mpsc;

use crate::{
    DashboardAuthPolicy, ManagementApiState, ManagementAuthState, ManagementFileDownloadState,
    ManagementStatusResponse, PluginMarketManagementState, management_router,
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
