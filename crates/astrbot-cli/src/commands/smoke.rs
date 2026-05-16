use std::error::Error;
use std::path::PathBuf;

use astrbot_runtime::{AstrbotRuntime, RuntimeConfig};

pub(super) async fn smoke(config_path: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
    let config = match config_path {
        Some(path) => RuntimeConfig::from_json_file(path)?,
        None => RuntimeConfig::from_env(),
    };
    let mut runtime = AstrbotRuntime::initialize(config)?;

    runtime
        .emit_mock_text("cli-event-1", "cli-conversation", "cli-user", "hello")
        .await?;
    runtime.run_once().await?;

    for sent in runtime.sent_messages().await {
        println!(
            "[{}] {}",
            sent.session.conversation_id,
            sent.chain.plain_text()
        );
    }

    Ok(())
}
