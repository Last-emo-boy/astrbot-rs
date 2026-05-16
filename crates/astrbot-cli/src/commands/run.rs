use std::error::Error;
use std::path::PathBuf;

use astrbot_runtime::{AstrbotRuntime, RuntimeConfig};

use crate::webchat_server::{PendingWebChatServer, prepare_webchat_server};

pub(super) async fn run(config_path: PathBuf) -> Result<(), Box<dyn Error>> {
    let config = RuntimeConfig::from_json_file(&config_path)?;
    let webchat_server_config = config.webchat_server.clone();
    let runtime = AstrbotRuntime::initialize(config)?;
    let pending_webchat_server = prepare_webchat_server(&runtime, &webchat_server_config).await?;
    let handle = runtime.start();
    let webchat_server = pending_webchat_server.map(PendingWebChatServer::start);

    println!("AstrBot runtime started. Press Ctrl+C to stop.");
    if let Some(server) = &webchat_server {
        println!(
            "WebChat HTTP server listening on http://{}",
            server.address()
        );
    }

    tokio::signal::ctrl_c().await?;
    if let Some(server) = webchat_server {
        server.stop().await?;
    }
    handle.stop().await?;
    Ok(())
}
