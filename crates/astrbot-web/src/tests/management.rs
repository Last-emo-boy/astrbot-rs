use std::sync::Arc;

use astrbot_core::{MessageChain, MessageEvent, Result};
use astrbot_platform::{PlatformBuildContext, PlatformConfig, PlatformManager, PlatformRegistry};
use astrbot_plugin::{
    HandlerMetadata, PluginControl, PluginEventType, PluginHandler, PluginRegistry,
    RegisteredHandler,
};
use astrbot_provider::{
    ChatProviderConfig, ProviderManager, ProviderManagerConfigSet, ProviderRegistry,
};
use async_trait::async_trait;
use axum::http::StatusCode;
use tokio::sync::mpsc;

use crate::{ManagementApiState, ManagementStatusResponse, management_router};

use super::support::{get, response_json};

#[tokio::test]
async fn management_status_reads_provider_platform_and_plugin_facades() {
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

    let router = management_router(ManagementApiState::from_managers(
        &provider_manager,
        &platform_manager,
        &plugin_registry,
    ));

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
