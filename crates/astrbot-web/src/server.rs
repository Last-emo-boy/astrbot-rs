use std::sync::Arc;
use std::{future::Future, io};

use astrbot_platform::WebChatPlatform;
use tokio::net::TcpListener;

use crate::management::{
    ManagementApiState, ManagementAuthState, management_router, management_router_with_auth,
};
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
