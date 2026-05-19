use std::sync::Arc;
use std::{future::Future, io};

use astrbot_platform::WebChatPlatform;
use astrbot_runtime::DashboardAssetSelection;
use axum::Router;
use tokio::net::TcpListener;

use crate::dashboard::dashboard_static_router;
use crate::management::{
    ManagementApiState, ManagementAuthState, management_router, management_router_with_auth,
};
use crate::openapi::openapi_chat_router;
use crate::routes::webchat_router;

pub async fn serve_webchat(listener: TcpListener, webchat: Arc<WebChatPlatform>) -> io::Result<()> {
    axum::serve(listener, webchat_router(webchat)).await
}

pub async fn serve_webchat_with_shutdown<F>(
    listener: TcpListener,
    webchat: Arc<WebChatPlatform>,
    shutdown: F,
) -> io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(listener, webchat_router(webchat))
        .with_graceful_shutdown(shutdown)
        .await
}

pub async fn serve_management(listener: TcpListener, state: ManagementApiState) -> io::Result<()> {
    axum::serve(listener, management_router(state)).await
}

pub async fn serve_management_with_auth(
    listener: TcpListener,
    state: ManagementApiState,
    auth: ManagementAuthState,
) -> io::Result<()> {
    axum::serve(listener, management_router_with_auth(state, auth)).await
}

pub async fn serve_management_with_shutdown<F>(
    listener: TcpListener,
    state: ManagementApiState,
    shutdown: F,
) -> io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(listener, management_router(state))
        .with_graceful_shutdown(shutdown)
        .await
}

pub async fn serve_management_with_auth_and_shutdown<F>(
    listener: TcpListener,
    state: ManagementApiState,
    auth: ManagementAuthState,
    shutdown: F,
) -> io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(listener, management_router_with_auth(state, auth))
        .with_graceful_shutdown(shutdown)
        .await
}

pub fn dashboard_router(
    webchat: Arc<WebChatPlatform>,
    management: ManagementApiState,
    assets: DashboardAssetSelection,
) -> Router {
    let openapi_api_keys = management.api_keys().cloned();
    management_router(management)
        .merge(openapi_chat_router(webchat.clone(), openapi_api_keys))
        .merge(webchat_router(webchat))
        .merge(dashboard_static_router(assets))
}

pub fn dashboard_router_with_auth(
    webchat: Arc<WebChatPlatform>,
    management: ManagementApiState,
    auth: ManagementAuthState,
    assets: DashboardAssetSelection,
) -> Router {
    let openapi_api_keys = management.api_keys().cloned();
    management_router_with_auth(management, auth)
        .merge(openapi_chat_router(webchat.clone(), openapi_api_keys))
        .merge(webchat_router(webchat))
        .merge(dashboard_static_router(assets))
}

pub async fn serve_dashboard(
    listener: TcpListener,
    webchat: Arc<WebChatPlatform>,
    management: ManagementApiState,
    assets: DashboardAssetSelection,
) -> io::Result<()> {
    axum::serve(listener, dashboard_router(webchat, management, assets)).await
}

pub async fn serve_dashboard_with_shutdown<F>(
    listener: TcpListener,
    webchat: Arc<WebChatPlatform>,
    management: ManagementApiState,
    assets: DashboardAssetSelection,
    shutdown: F,
) -> io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(listener, dashboard_router(webchat, management, assets))
        .with_graceful_shutdown(shutdown)
        .await
}

pub async fn serve_dashboard_with_auth_and_shutdown<F>(
    listener: TcpListener,
    webchat: Arc<WebChatPlatform>,
    management: ManagementApiState,
    auth: ManagementAuthState,
    assets: DashboardAssetSelection,
    shutdown: F,
) -> io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(
        listener,
        dashboard_router_with_auth(webchat, management, auth, assets),
    )
    .with_graceful_shutdown(shutdown)
    .await
}
