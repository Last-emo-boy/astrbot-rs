use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

pub const DEFAULT_PERSONA_ID: &str = "default";
const NONE_PERSONA_SENTINEL: &str = "[%None]";
const WEBCHAT_DEFAULT_PERSONA_ID: &str = "_chatui_default_";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonaProfile {
    pub id: String,
    pub system_prompt: String,
    pub begin_dialogs: Vec<PersonaDialogTurn>,
    pub tools: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
    pub custom_error_message: Option<String>,
    pub folder_id: Option<String>,
}

impl PersonaProfile {
    pub fn new(id: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            system_prompt: system_prompt.into(),
            begin_dialogs: Vec::new(),
            tools: None,
            skills: None,
            custom_error_message: None,
            folder_id: None,
        }
    }

    pub fn with_begin_dialog(
        mut self,
        role: PersonaDialogRole,
        content: impl Into<String>,
    ) -> Self {
        self.begin_dialogs
            .push(PersonaDialogTurn::new(role, content));
        self
    }

    pub fn with_tools(mut self, tools: Option<Vec<String>>) -> Self {
        self.tools = tools;
        self
    }

    pub fn with_skills(mut self, skills: Option<Vec<String>>) -> Self {
        self.skills = skills;
        self
    }

    pub fn with_folder_id(mut self, folder_id: impl Into<String>) -> Self {
        self.folder_id = Some(folder_id.into());
        self
    }

    pub fn with_custom_error_message(mut self, message: impl Into<String>) -> Self {
        let message = message.into();
        self.custom_error_message = (!message.trim().is_empty()).then_some(message);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonaDialogRole {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonaDialogTurn {
    pub role: PersonaDialogRole,
    pub content: String,
}

impl PersonaDialogTurn {
    pub fn new(role: PersonaDialogRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonaFolder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub description: Option<String>,
    pub sort_order: i32,
}

impl PersonaFolder {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            parent_id: None,
            description: None,
            sort_order: 0,
        }
    }

    pub fn with_parent_id(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.trim().is_empty()).then_some(description);
        self
    }

    pub fn with_sort_order(mut self, sort_order: i32) -> Self {
        self.sort_order = sort_order;
        self
    }
}

#[async_trait]
pub trait PersonaRepository: Send + Sync {
    async fn upsert_persona(&self, persona: PersonaProfile) -> Result<()>;

    async fn persona(&self, persona_id: &str) -> Result<Option<PersonaProfile>>;

    async fn list_personas(&self) -> Result<Vec<PersonaProfile>>;

    async fn upsert_folder(&self, folder: PersonaFolder) -> Result<()>;

    async fn folder(&self, folder_id: &str) -> Result<Option<PersonaFolder>>;

    async fn folders_by_parent(&self, parent_id: Option<&str>) -> Result<Vec<PersonaFolder>>;
}

#[derive(Default)]
pub struct InMemoryPersonaRepository {
    personas: RwLock<HashMap<String, PersonaProfile>>,
    folders: RwLock<HashMap<String, PersonaFolder>>,
}

impl InMemoryPersonaRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PersonaRepository for InMemoryPersonaRepository {
    async fn upsert_persona(&self, persona: PersonaProfile) -> Result<()> {
        self.personas
            .write()
            .map_err(lock_error)?
            .insert(persona.id.clone(), persona);
        Ok(())
    }

    async fn persona(&self, persona_id: &str) -> Result<Option<PersonaProfile>> {
        Ok(self
            .personas
            .read()
            .map_err(lock_error)?
            .get(persona_id)
            .cloned())
    }

    async fn list_personas(&self) -> Result<Vec<PersonaProfile>> {
        let mut personas = self
            .personas
            .read()
            .map_err(lock_error)?
            .values()
            .cloned()
            .collect::<Vec<_>>();
        personas.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(personas)
    }

    async fn upsert_folder(&self, folder: PersonaFolder) -> Result<()> {
        self.folders
            .write()
            .map_err(lock_error)?
            .insert(folder.id.clone(), folder);
        Ok(())
    }

    async fn folder(&self, folder_id: &str) -> Result<Option<PersonaFolder>> {
        Ok(self
            .folders
            .read()
            .map_err(lock_error)?
            .get(folder_id)
            .cloned())
    }

    async fn folders_by_parent(&self, parent_id: Option<&str>) -> Result<Vec<PersonaFolder>> {
        let mut folders = self
            .folders
            .read()
            .map_err(lock_error)?
            .values()
            .filter(|folder| folder.parent_id.as_deref() == parent_id)
            .cloned()
            .collect::<Vec<_>>();
        folders.sort_by(|left, right| {
            left.sort_order
                .cmp(&right.sort_order)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(folders)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PersonaResolveRequest {
    pub session_id: Option<String>,
    pub platform_name: Option<String>,
    pub forced_persona_id: Option<String>,
    pub conversation_persona_id: Option<String>,
    pub provider_default_persona_id: Option<String>,
}

impl PersonaResolveRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_platform_name(mut self, platform_name: impl Into<String>) -> Self {
        self.platform_name = Some(platform_name.into());
        self
    }

    pub fn with_forced_persona_id(mut self, persona_id: impl Into<String>) -> Self {
        self.forced_persona_id = Some(persona_id.into());
        self
    }

    pub fn with_conversation_persona_id(mut self, persona_id: impl Into<String>) -> Self {
        self.conversation_persona_id = Some(persona_id.into());
        self
    }

    pub fn with_provider_default_persona_id(mut self, persona_id: impl Into<String>) -> Self {
        self.provider_default_persona_id = Some(persona_id.into());
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonaResolveSource {
    ForcedSession,
    Conversation,
    ProviderDefault,
    Default,
    WebChatDefault,
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPersona {
    pub persona_id: Option<String>,
    pub profile: Option<PersonaProfile>,
    pub source: PersonaResolveSource,
}

#[derive(Clone)]
pub struct PersonaManager {
    repository: Arc<dyn PersonaRepository>,
    default_persona: PersonaProfile,
}

impl Default for PersonaManager {
    fn default() -> Self {
        Self::new(
            DEFAULT_PERSONA_ID,
            "You are a helpful and friendly assistant.",
        )
    }
}

impl PersonaManager {
    pub fn new(default_id: impl Into<String>, default_prompt: impl Into<String>) -> Self {
        Self::with_repository(
            Arc::new(InMemoryPersonaRepository::new()),
            PersonaProfile::new(default_id, default_prompt),
        )
    }

    pub fn with_repository(
        repository: Arc<dyn PersonaRepository>,
        default_persona: PersonaProfile,
    ) -> Self {
        Self {
            repository,
            default_persona,
        }
    }

    pub async fn upsert_persona(&self, persona: PersonaProfile) -> Result<()> {
        self.repository.upsert_persona(persona).await
    }

    pub async fn upsert_folder(&self, folder: PersonaFolder) -> Result<()> {
        self.repository.upsert_folder(folder).await
    }

    pub async fn persona(&self, persona_id: &str) -> Result<Option<PersonaProfile>> {
        self.repository.persona(persona_id).await
    }

    pub async fn personas_by_folder(&self, folder_id: Option<&str>) -> Result<Vec<PersonaProfile>> {
        let mut personas = self
            .repository
            .list_personas()
            .await?
            .into_iter()
            .filter(|persona| persona.folder_id.as_deref() == folder_id)
            .collect::<Vec<_>>();
        personas.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(personas)
    }

    pub async fn folders_by_parent(&self, parent_id: Option<&str>) -> Result<Vec<PersonaFolder>> {
        self.repository.folders_by_parent(parent_id).await
    }

    pub async fn resolve(&self, request: &PersonaResolveRequest) -> Result<ResolvedPersona> {
        if is_none_persona(request.forced_persona_id.as_deref()) {
            return Ok(ResolvedPersona {
                persona_id: None,
                profile: None,
                source: PersonaResolveSource::Disabled,
            });
        }

        let candidates = [
            (
                request.forced_persona_id.as_deref(),
                PersonaResolveSource::ForcedSession,
            ),
            (
                request.conversation_persona_id.as_deref(),
                PersonaResolveSource::Conversation,
            ),
            (
                request.provider_default_persona_id.as_deref(),
                PersonaResolveSource::ProviderDefault,
            ),
        ];

        for (persona_id, source) in candidates {
            if is_none_persona(persona_id) {
                return Ok(ResolvedPersona {
                    persona_id: None,
                    profile: None,
                    source: PersonaResolveSource::Disabled,
                });
            }
            let Some(persona_id) = non_empty_str(persona_id) else {
                continue;
            };
            if let Some(profile) = self.repository.persona(persona_id).await? {
                return Ok(ResolvedPersona {
                    persona_id: Some(profile.id.clone()),
                    profile: Some(profile),
                    source,
                });
            }
        }

        if request.platform_name.as_deref() == Some("webchat") {
            return Ok(ResolvedPersona {
                persona_id: Some(WEBCHAT_DEFAULT_PERSONA_ID.to_string()),
                profile: None,
                source: PersonaResolveSource::WebChatDefault,
            });
        }

        Ok(ResolvedPersona {
            persona_id: Some(self.default_persona.id.clone()),
            profile: Some(self.default_persona.clone()),
            source: PersonaResolveSource::Default,
        })
    }
}

fn non_empty_str(value: Option<&str>) -> Option<&str> {
    value.and_then(|value| (!value.trim().is_empty()).then_some(value))
}

fn is_none_persona(value: Option<&str>) -> bool {
    value == Some(NONE_PERSONA_SENTINEL)
}

fn lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("persona manager lock: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{
        PersonaDialogRole, PersonaFolder, PersonaManager, PersonaProfile, PersonaResolveRequest,
        PersonaResolveSource,
    };

    #[tokio::test]
    async fn persona_manager_resolves_forced_conversation_and_default_personas() {
        let manager = PersonaManager::default();
        manager
            .upsert_persona(PersonaProfile::new("creative", "be vivid"))
            .await
            .expect("persona should save");
        manager
            .upsert_persona(PersonaProfile::new("support", "be concise"))
            .await
            .expect("persona should save");

        let forced = manager
            .resolve(
                &PersonaResolveRequest::new()
                    .with_forced_persona_id("creative")
                    .with_conversation_persona_id("support"),
            )
            .await
            .expect("persona should resolve");
        assert_eq!(forced.persona_id.as_deref(), Some("creative"));
        assert_eq!(forced.source, PersonaResolveSource::ForcedSession);

        let conversation = manager
            .resolve(&PersonaResolveRequest::new().with_conversation_persona_id("support"))
            .await
            .expect("persona should resolve");
        assert_eq!(conversation.persona_id.as_deref(), Some("support"));
        assert_eq!(conversation.source, PersonaResolveSource::Conversation);

        let default = manager
            .resolve(&PersonaResolveRequest::new())
            .await
            .expect("persona should resolve");
        assert_eq!(default.source, PersonaResolveSource::Default);
    }

    #[tokio::test]
    async fn persona_manager_groups_personas_by_folder_metadata() {
        let manager = PersonaManager::default();
        manager
            .upsert_folder(PersonaFolder::new("root-a", "Root A").with_sort_order(10))
            .await
            .expect("folder should save");
        manager
            .upsert_persona(
                PersonaProfile::new("analyst", "be rigorous")
                    .with_folder_id("root-a")
                    .with_begin_dialog(PersonaDialogRole::User, "hello"),
            )
            .await
            .expect("persona should save");

        let personas = manager
            .personas_by_folder(Some("root-a"))
            .await
            .expect("personas should load");
        let folders = manager
            .folders_by_parent(None)
            .await
            .expect("folders should load");

        assert_eq!(personas[0].id, "analyst");
        assert_eq!(personas[0].begin_dialogs[0].content, "hello");
        assert_eq!(folders[0].id, "root-a");
    }

    #[tokio::test]
    async fn persona_manager_respects_none_and_webchat_special_defaults() {
        let manager = PersonaManager::default();

        let disabled = manager
            .resolve(&PersonaResolveRequest::new().with_conversation_persona_id("[%None]"))
            .await
            .expect("persona should resolve");
        assert_eq!(disabled.source, PersonaResolveSource::Disabled);
        assert!(disabled.profile.is_none());

        let webchat = manager
            .resolve(
                &PersonaResolveRequest::new()
                    .with_platform_name("webchat")
                    .with_provider_default_persona_id("missing"),
            )
            .await
            .expect("persona should resolve");
        assert_eq!(webchat.source, PersonaResolveSource::WebChatDefault);
        assert_eq!(webchat.persona_id.as_deref(), Some("_chatui_default_"));
    }
}
