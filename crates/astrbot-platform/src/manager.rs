use std::collections::HashMap;
use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};
use astrbot_observability::{
    ComponentKind, ComponentStatus, NoopStatusEventSink, StatusEvent, StatusEventSink,
    StatusSeverity,
};
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
    status_sink: Arc<dyn StatusEventSink>,
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
            status_sink: Arc::new(NoopStatusEventSink),
        })
    }

    pub fn with_status_sink(mut self, status_sink: Arc<dyn StatusEventSink>) -> Self {
        self.status_sink = status_sink;
        self
    }

    pub fn platform_count(&self) -> usize {
        self.adapters.len()
    }

    pub fn platform_ids(&self) -> Vec<String> {
        let mut ids = self.adapters.keys().cloned().collect::<Vec<_>>();
        ids.sort();
        ids
    }

    pub fn recording_sink_count(&self) -> usize {
        self.recording_sinks.len()
    }

    pub fn mock_platform_count(&self) -> usize {
        self.mock_platforms.len()
    }

    pub fn webchat_platform_count(&self) -> usize {
        self.webchat_platforms.len()
    }

    pub fn onebot_platform_count(&self) -> usize {
        self.onebot_platforms.len()
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
            .iter()
            .map(|(platform_id, adapter)| {
                let adapter = Arc::clone(adapter);
                let platform_id = platform_id.clone();
                let status_sink = self.status_sink.clone();
                tokio::spawn(async move {
                    status_sink.emit(
                        StatusEvent::new(ComponentKind::Platform, ComponentStatus::Starting)
                            .with_component_id(platform_id.clone()),
                    );
                    let result = adapter.run().await;
                    let status = if result.is_ok() {
                        ComponentStatus::Stopped
                    } else {
                        ComponentStatus::Failed
                    };
                    let mut event = StatusEvent::new(ComponentKind::Platform, status)
                        .with_component_id(platform_id);
                    if let Err(err) = &result {
                        event = event
                            .with_severity(StatusSeverity::Error)
                            .with_message(err.to_string());
                    }
                    status_sink.emit(event);
                    result
                })
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
            self.status_sink.emit(
                StatusEvent::new(ComponentKind::Platform, ComponentStatus::Stopping)
                    .with_component_id(platform_id.clone()),
            );
            adapter.terminate().await.map_err(|err| {
                AstrbotError::Platform(format!("terminate platform {platform_id}: {err}"))
            })?;
            self.status_sink.emit(
                StatusEvent::new(ComponentKind::Platform, ComponentStatus::Stopped)
                    .with_component_id(platform_id.clone()),
            );
        }
        Ok(())
    }
}
