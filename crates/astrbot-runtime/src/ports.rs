use std::collections::HashSet;

use astrbot_core::{MessageEvent, Result};
use astrbot_pipeline::SessionStatusPort;
use async_trait::async_trait;

use crate::RuntimeSessionStatusConfig;
pub(crate) struct ConfiguredSessionStatusPort {
    disabled_sessions: HashSet<String>,
}

impl ConfiguredSessionStatusPort {
    pub(crate) fn new(config: RuntimeSessionStatusConfig) -> Self {
        Self {
            disabled_sessions: config
                .disabled_sessions
                .into_iter()
                .map(|session: String| session.trim().to_string())
                .filter(|session| !session.is_empty())
                .collect(),
        }
    }
}

#[async_trait]
impl SessionStatusPort for ConfiguredSessionStatusPort {
    async fn is_session_enabled(&self, event: &MessageEvent) -> Result<bool> {
        if self.disabled_sessions.is_empty() {
            return Ok(true);
        }

        let platform_session = format!(
            "{}:{}",
            event.session.platform_id, event.session.conversation_id
        );
        Ok(!self
            .disabled_sessions
            .contains(&event.session.conversation_id)
            && !self.disabled_sessions.contains(&platform_session)
            && !self.disabled_sessions.contains(&event.sender.id))
    }
}
