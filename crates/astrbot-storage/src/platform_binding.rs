use std::collections::HashMap;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, MessageSession, Result};
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformRoutingBindingRecord {
    pub session: MessageSession,
    pub platform_session_id: String,
    pub scene: String,
    pub last_inbound_message_id: Option<String>,
    pub last_outbound_message_id: Option<String>,
    pub sender_id: Option<String>,
    pub sender_binding_key: Option<String>,
    pub sender_binding_value: Option<String>,
    pub updated_at_unix: Option<u64>,
}

impl PlatformRoutingBindingRecord {
    pub fn new(
        session: MessageSession,
        platform_session_id: impl Into<String>,
        scene: impl Into<String>,
    ) -> Self {
        Self {
            session,
            platform_session_id: platform_session_id.into(),
            scene: scene.into(),
            last_inbound_message_id: None,
            last_outbound_message_id: None,
            sender_id: None,
            sender_binding_key: None,
            sender_binding_value: None,
            updated_at_unix: None,
        }
    }

    pub fn with_last_inbound_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.last_inbound_message_id = non_empty_string(message_id);
        self
    }

    pub fn with_last_outbound_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.last_outbound_message_id = non_empty_string(message_id);
        self
    }

    pub fn with_sender_binding(
        mut self,
        sender_id: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.sender_id = non_empty_string(sender_id);
        self.sender_binding_key = non_empty_string(key);
        self.sender_binding_value = non_empty_string(value);
        self
    }

    pub fn updated_at_unix(mut self, updated_at_unix: u64) -> Self {
        self.updated_at_unix = Some(updated_at_unix);
        self
    }

    fn key(&self) -> PlatformRoutingBindingKey {
        PlatformRoutingBindingKey::new(&self.session.platform_id, &self.session.conversation_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PlatformRoutingBindingKey {
    platform_id: String,
    conversation_id: String,
}

impl PlatformRoutingBindingKey {
    fn new(platform_id: &str, conversation_id: &str) -> Self {
        Self {
            platform_id: platform_id.trim().to_string(),
            conversation_id: conversation_id.trim().to_string(),
        }
    }

    fn validate(&self) -> Result<()> {
        if self.platform_id.is_empty() || self.conversation_id.is_empty() {
            return Err(AstrbotError::Pipeline(
                "platform routing binding key must include platform_id and conversation_id"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
pub trait PlatformRoutingBindingRepository: Send + Sync {
    async fn put_platform_binding(&self, record: PlatformRoutingBindingRecord) -> Result<()>;

    async fn platform_binding(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<Option<PlatformRoutingBindingRecord>>;

    async fn platform_bindings(
        &self,
        platform_id: &str,
    ) -> Result<Vec<PlatformRoutingBindingRecord>>;

    async fn remove_platform_binding(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<Option<PlatformRoutingBindingRecord>>;
}

#[derive(Default)]
pub struct InMemoryPlatformRoutingBindingRepository {
    bindings: RwLock<HashMap<PlatformRoutingBindingKey, PlatformRoutingBindingRecord>>,
}

impl InMemoryPlatformRoutingBindingRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PlatformRoutingBindingRepository for InMemoryPlatformRoutingBindingRepository {
    async fn put_platform_binding(&self, record: PlatformRoutingBindingRecord) -> Result<()> {
        let key = record.key();
        key.validate()?;
        self.bindings
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("platform binding lock: {err}")))?
            .insert(key, record);
        Ok(())
    }

    async fn platform_binding(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<Option<PlatformRoutingBindingRecord>> {
        let key = PlatformRoutingBindingKey::new(platform_id, conversation_id);
        key.validate()?;
        Ok(self
            .bindings
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("platform binding lock: {err}")))?
            .get(&key)
            .cloned())
    }

    async fn platform_bindings(
        &self,
        platform_id: &str,
    ) -> Result<Vec<PlatformRoutingBindingRecord>> {
        let platform_id = platform_id.trim();
        if platform_id.is_empty() {
            return Err(AstrbotError::Pipeline(
                "platform id must not be empty".to_string(),
            ));
        }
        Ok(self
            .bindings
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("platform binding lock: {err}")))?
            .iter()
            .filter(|(key, _)| key.platform_id == platform_id)
            .map(|(_, record)| record.clone())
            .collect())
    }

    async fn remove_platform_binding(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<Option<PlatformRoutingBindingRecord>> {
        let key = PlatformRoutingBindingKey::new(platform_id, conversation_id);
        key.validate()?;
        Ok(self
            .bindings
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("platform binding lock: {err}")))?
            .remove(&key))
    }
}

fn non_empty_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use astrbot_core::MessageSession;

    use super::{
        InMemoryPlatformRoutingBindingRepository, PlatformRoutingBindingRecord,
        PlatformRoutingBindingRepository,
    };

    #[tokio::test]
    async fn platform_binding_repository_stores_recent_message_route_state() {
        let repository = InMemoryPlatformRoutingBindingRepository::new();
        let session = MessageSession::group("qq-official", "group:g1");
        repository
            .put_platform_binding(
                PlatformRoutingBindingRecord::new(session, "g1", "group")
                    .with_last_inbound_message_id("msg-1")
                    .with_last_outbound_message_id("msg-2")
                    .updated_at_unix(100),
            )
            .await
            .expect("binding should store");

        let binding = repository
            .platform_binding("qq-official", "group:g1")
            .await
            .expect("binding lookup should work")
            .expect("binding should exist");

        assert_eq!(binding.platform_session_id, "g1");
        assert_eq!(binding.scene, "group");
        assert_eq!(binding.last_inbound_message_id.as_deref(), Some("msg-1"));
        assert_eq!(binding.last_outbound_message_id.as_deref(), Some("msg-2"));
    }

    #[tokio::test]
    async fn platform_binding_repository_stores_sender_external_ids() {
        let repository = InMemoryPlatformRoutingBindingRepository::new();
        repository
            .put_platform_binding(
                PlatformRoutingBindingRecord::new(
                    MessageSession::new("dingtalk", "private:user-1"),
                    "user-1",
                    "direct",
                )
                .with_sender_binding("user-1", "dingtalk_staff_id", "staff-1"),
            )
            .await
            .expect("binding should store");

        let bindings = repository
            .platform_bindings("dingtalk")
            .await
            .expect("bindings should load");

        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].sender_id.as_deref(), Some("user-1"));
        assert_eq!(
            bindings[0].sender_binding_key.as_deref(),
            Some("dingtalk_staff_id")
        );
        assert_eq!(bindings[0].sender_binding_value.as_deref(), Some("staff-1"));
    }
}
