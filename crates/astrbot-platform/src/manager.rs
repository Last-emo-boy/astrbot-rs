use std::collections::HashMap;
use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};
use tokio::task::JoinHandle;

use crate::{
    MessageRecorder, MockPlatform, OneBotPlatform, PlatformAdapter, PlatformBuildContext,
    PlatformConfig, PlatformRegistry, WebChatPlatform, validate_platform_id,
};
pub struct PlatformManager {
    adapters: HashMap<String, Arc<dyn PlatformAdapter>>,
    recording_sinks: HashMap<String, Arc<dyn MessageRecorder>>,
    mock_platforms: HashMap<String, Arc<MockPlatform>>,
    webchat_platforms: HashMap<String, Arc<WebChatPlatform>>,
    onebot_platforms: HashMap<String, Arc<OneBotPlatform>>,
}

impl PlatformManager {
    pub fn from_configs(
        registry: &PlatformRegistry,
        configs: impl IntoIterator<Item = PlatformConfig>,
        ctx: PlatformBuildContext,
    ) -> Result<Self> {
        let mut adapters = HashMap::new();
        let mut recording_sinks = HashMap::new();
        let mut mock_platforms = HashMap::new();
        let mut webchat_platforms = HashMap::new();
        let mut onebot_platforms = HashMap::new();

        for config in configs {
            if !config.enabled {
                continue;
            }
            validate_platform_id(&config.id)?;
            if adapters.contains_key(&config.id) {
                return Err(AstrbotError::Platform(format!(
                    "platform id {} is already configured",
                    config.id
                )));
            }

            let built = registry.build_platform(&config, &ctx)?;
            if let Some(sink) = built.recording_sink {
                recording_sinks.insert(config.id.clone(), sink);
            }
            if let Some(mock_platform) = built.mock_platform {
                mock_platforms.insert(config.id.clone(), mock_platform);
            }
            if let Some(webchat_platform) = built.webchat_platform {
                webchat_platforms.insert(config.id.clone(), webchat_platform);
            }
            if let Some(onebot_platform) = built.onebot_platform {
                onebot_platforms.insert(config.id.clone(), onebot_platform);
            }
            adapters.insert(config.id, built.adapter);
        }

        Ok(Self {
            adapters,
            recording_sinks,
            mock_platforms,
            webchat_platforms,
            onebot_platforms,
        })
    }

    pub fn platform_count(&self) -> usize {
        self.adapters.len()
    }

    pub fn adapter(&self, id: &str) -> Option<Arc<dyn PlatformAdapter>> {
        self.adapters.get(id).cloned()
    }

    pub fn recording_sink(&self, id: &str) -> Option<Arc<dyn MessageRecorder>> {
        self.recording_sinks.get(id).cloned()
    }

    pub fn mock_platform(&self, id: &str) -> Option<Arc<MockPlatform>> {
        self.mock_platforms.get(id).cloned()
    }

    pub fn webchat_platform(&self, id: &str) -> Option<Arc<WebChatPlatform>> {
        self.webchat_platforms.get(id).cloned()
    }

    pub fn onebot_platform(&self, id: &str) -> Option<Arc<OneBotPlatform>> {
        self.onebot_platforms.get(id).cloned()
    }

    pub fn spawn_all(&self) -> Vec<JoinHandle<Result<()>>> {
        self.adapters
            .values()
            .map(|adapter| {
                let adapter = Arc::clone(adapter);
                tokio::spawn(async move { adapter.run().await })
            })
            .collect()
    }

    pub async fn run_all(&self) -> Result<()> {
        for task in self.spawn_all() {
            match task.await {
                Ok(result) => result?,
                Err(err) => {
                    return Err(AstrbotError::Platform(format!(
                        "platform task join failed: {err}"
                    )));
                }
            }
        }
        Ok(())
    }

    pub async fn terminate(&self) -> Result<()> {
        for (platform_id, adapter) in &self.adapters {
            adapter.terminate().await.map_err(|err| {
                AstrbotError::Platform(format!("terminate platform {platform_id}: {err}"))
            })?;
        }
        Ok(())
    }
}
