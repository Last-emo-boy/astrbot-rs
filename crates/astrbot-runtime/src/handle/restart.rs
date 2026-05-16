use std::collections::HashMap;

use astrbot_core::Result;

use crate::RuntimeConfig;

use super::runtime::{AstrbotRuntime, RuntimeHandle};

struct RestartState {
    provider_preferences: Option<HashMap<String, String>>,
    status_sink: astrbot_observability::SharedStatusEventSink,
}

impl RestartState {
    async fn capture(handle: &RuntimeHandle, config: &RuntimeConfig) -> Result<Self> {
        let provider_preferences = if config.state_policy.preserve_provider_preference_on_restart {
            Some(handle.provider_preference.snapshot().await?)
        } else {
            None
        };

        Ok(Self {
            provider_preferences,
            status_sink: handle.status_sink.clone(),
        })
    }

    async fn restore(self, runtime: AstrbotRuntime) -> Result<AstrbotRuntime> {
        if let Some(preferences) = self.provider_preferences {
            runtime
                .provider_preference
                .replace_with(preferences)
                .await?;
        }
        Ok(runtime.with_status_sink(self.status_sink))
    }
}

impl RuntimeHandle {
    pub async fn restart(self, config: RuntimeConfig) -> Result<RuntimeHandle> {
        let restart_state = RestartState::capture(&self, &config).await?;

        self.stop().await?;
        let runtime = AstrbotRuntime::initialize(config)?;
        let runtime = restart_state.restore(runtime).await?;
        Ok(runtime.start())
    }
}
