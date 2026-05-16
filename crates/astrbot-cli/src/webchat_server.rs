use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use astrbot_platform::WebChatPlatform;
use astrbot_runtime::{AstrbotRuntime, RuntimeWebChatServerConfig};
use astrbot_web::serve_webchat_with_shutdown;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

pub(crate) async fn prepare_webchat_server(
    runtime: &AstrbotRuntime,
    config: &RuntimeWebChatServerConfig,
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

    Ok(Some(PendingWebChatServer {
        listener,
        webchat,
        address,
    }))
}

pub(crate) struct PendingWebChatServer {
    listener: TcpListener,
    webchat: Arc<WebChatPlatform>,
    pub(crate) address: SocketAddr,
}

impl PendingWebChatServer {
    pub(crate) fn start(self) -> WebChatServerHandle {
        let address = self.address;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(serve_webchat_with_shutdown(
            self.listener,
            self.webchat,
            async move {
                let _ = shutdown_rx.await;
            },
        ));

        WebChatServerHandle {
            address,
            shutdown_tx: Some(shutdown_tx),
            task,
        }
    }
}

pub(crate) struct WebChatServerHandle {
    address: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<io::Result<()>>,
}

impl WebChatServerHandle {
    pub(crate) fn address(&self) -> SocketAddr {
        self.address
    }

    pub(crate) async fn stop(mut self) -> Result<(), Box<dyn Error>> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }

        let result = self
            .task
            .await
            .map_err(|err| io::Error::other(format!("webchat server task join failed: {err}")))?;
        result?;
        Ok(())
    }
}
