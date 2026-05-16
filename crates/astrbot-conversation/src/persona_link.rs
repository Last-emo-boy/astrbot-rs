use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonaConversationLink {
    pub platform_id: String,
    pub conversation_id: String,
    pub persona_id: String,
}

impl PersonaConversationLink {
    pub fn new(
        platform_id: impl Into<String>,
        conversation_id: impl Into<String>,
        persona_id: impl Into<String>,
    ) -> Self {
        Self {
            platform_id: platform_id.into(),
            conversation_id: conversation_id.into(),
            persona_id: persona_id.into(),
        }
    }
}

#[async_trait]
pub trait PersonaConversationLinkRepository: Send + Sync {
    async fn set_link(&self, link: PersonaConversationLink) -> Result<()>;

    async fn link_for_conversation(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<Option<PersonaConversationLink>>;

    async fn remove_link(&self, platform_id: &str, conversation_id: &str) -> Result<bool>;
}

#[derive(Default)]
pub struct InMemoryPersonaConversationLinkRepository {
    links: RwLock<HashMap<PersonaConversationKey, PersonaConversationLink>>,
}

impl InMemoryPersonaConversationLinkRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PersonaConversationLinkRepository for InMemoryPersonaConversationLinkRepository {
    async fn set_link(&self, link: PersonaConversationLink) -> Result<()> {
        let key = PersonaConversationKey::new(&link.platform_id, &link.conversation_id);
        self.links.write().map_err(lock_error)?.insert(key, link);
        Ok(())
    }

    async fn link_for_conversation(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<Option<PersonaConversationLink>> {
        Ok(self
            .links
            .read()
            .map_err(lock_error)?
            .get(&PersonaConversationKey::new(platform_id, conversation_id))
            .cloned())
    }

    async fn remove_link(&self, platform_id: &str, conversation_id: &str) -> Result<bool> {
        Ok(self
            .links
            .write()
            .map_err(lock_error)?
            .remove(&PersonaConversationKey::new(platform_id, conversation_id))
            .is_some())
    }
}

#[derive(Clone)]
pub struct PersonaConversationLinkService {
    repository: Arc<dyn PersonaConversationLinkRepository>,
}

impl Default for PersonaConversationLinkService {
    fn default() -> Self {
        Self::new()
    }
}

impl PersonaConversationLinkService {
    pub fn new() -> Self {
        Self::with_repository(Arc::new(InMemoryPersonaConversationLinkRepository::new()))
    }

    pub fn with_repository(repository: Arc<dyn PersonaConversationLinkRepository>) -> Self {
        Self { repository }
    }

    pub async fn set_link(&self, link: PersonaConversationLink) -> Result<()> {
        self.repository.set_link(link).await
    }

    pub async fn link_for_conversation(
        &self,
        platform_id: &str,
        conversation_id: &str,
    ) -> Result<Option<PersonaConversationLink>> {
        self.repository
            .link_for_conversation(platform_id, conversation_id)
            .await
    }

    pub async fn remove_link(&self, platform_id: &str, conversation_id: &str) -> Result<bool> {
        self.repository
            .remove_link(platform_id, conversation_id)
            .await
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PersonaConversationKey {
    platform_id: String,
    conversation_id: String,
}

impl PersonaConversationKey {
    fn new(platform_id: &str, conversation_id: &str) -> Self {
        Self {
            platform_id: platform_id.to_string(),
            conversation_id: conversation_id.to_string(),
        }
    }
}

fn lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("persona conversation link lock: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{PersonaConversationLink, PersonaConversationLinkService};

    #[tokio::test]
    async fn persona_link_service_replaces_and_removes_links() {
        let service = PersonaConversationLinkService::new();
        service
            .set_link(PersonaConversationLink::new(
                "webchat",
                "conversation-1",
                "persona-a",
            ))
            .await
            .expect("link should save");
        service
            .set_link(PersonaConversationLink::new(
                "webchat",
                "conversation-1",
                "persona-b",
            ))
            .await
            .expect("link should replace");

        let link = service
            .link_for_conversation("webchat", "conversation-1")
            .await
            .expect("link should load")
            .expect("link should exist");
        assert_eq!(link.persona_id, "persona-b");

        assert!(
            service
                .remove_link("webchat", "conversation-1")
                .await
                .expect("link should remove")
        );
        assert!(
            service
                .link_for_conversation("webchat", "conversation-1")
                .await
                .expect("link should load")
                .is_none()
        );
    }
}
