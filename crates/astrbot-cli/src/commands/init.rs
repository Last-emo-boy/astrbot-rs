use std::error::Error;
use std::fs;
use std::path::PathBuf;

use astrbot_runtime::{
    RuntimeConfig, RuntimeEmbeddingProviderConfig, RuntimePlatformConfig,
    RuntimeRerankProviderConfig, RuntimeWebChatServerConfig,
};

pub(super) async fn init(config_path: PathBuf) -> Result<(), Box<dyn Error>> {
    if !config_path.exists() {
        let mut config = RuntimeConfig::default();
        if !config
            .platforms
            .iter()
            .any(|platform| platform.id == "webchat")
        {
            config
                .platforms
                .push(RuntimePlatformConfig::webchat("webchat"));
        }
        if config.embedding_providers.is_empty() {
            config.embedding_providers = vec![RuntimeEmbeddingProviderConfig::mock("embedding", 2)];
            config.default_embedding_provider_id = Some("embedding".to_string());
        }
        if config.rerank_providers.is_empty() {
            config.rerank_providers = vec![RuntimeRerankProviderConfig::mock("rerank", 2)];
            config.default_rerank_provider_id = Some("rerank".to_string());
        }
        config.webchat_server = RuntimeWebChatServerConfig::enabled("webchat", "127.0.0.1", 6185);
        let content = serde_json::to_string_pretty(&config)?;
        fs::write(&config_path, content)?;
    }
    let _ = RuntimeConfig::from_json_file(&config_path)?;
    println!("initialized {}", config_path.display());
    Ok(())
}
