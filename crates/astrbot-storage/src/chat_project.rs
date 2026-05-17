use std::collections::HashMap;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

pub const DEFAULT_CHAT_PROJECT_EMOJI: &str = "📁";
pub const DEFAULT_CHAT_UI_PROJECT_EMOJI: &str = DEFAULT_CHAT_PROJECT_EMOJI;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatProjectRecord {
    pub project_id: String,
    pub creator: String,
    pub title: String,
    pub emoji: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ChatProjectRecord {
    pub fn new(
        project_id: impl Into<String>,
        creator: impl Into<String>,
        title: impl Into<String>,
        created_at: impl Into<String>,
    ) -> Self {
        let created_at = created_at.into();
        Self {
            project_id: project_id.into(),
            creator: creator.into(),
            title: title.into(),
            emoji: Some(DEFAULT_CHAT_PROJECT_EMOJI.to_string()),
            description: None,
            updated_at: created_at.clone(),
            created_at,
        }
    }

    pub fn with_emoji(mut self, emoji: Option<String>) -> Self {
        self.emoji = emoji.and_then(non_empty);
        self
    }

    pub fn with_description(mut self, description: Option<String>) -> Self {
        self.description = description.and_then(non_empty);
        self
    }

    pub fn with_updated_at(mut self, updated_at: impl Into<String>) -> Self {
        self.updated_at = updated_at.into();
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatProjectCreateRecord {
    pub creator: String,
    pub title: String,
    pub emoji: Option<String>,
    pub description: Option<String>,
    pub now: String,
}

impl ChatProjectCreateRecord {
    pub fn new(
        creator: impl Into<String>,
        title: impl Into<String>,
        now: impl Into<String>,
    ) -> Self {
        Self {
            creator: creator.into(),
            title: title.into(),
            emoji: Some(DEFAULT_CHAT_PROJECT_EMOJI.to_string()),
            description: None,
            now: now.into(),
        }
    }

    pub fn with_emoji(mut self, emoji: Option<String>) -> Self {
        if let Some(emoji) = emoji {
            self.emoji = non_empty(emoji);
        }
        self
    }

    pub fn with_description(mut self, description: Option<String>) -> Self {
        self.description = description.and_then(non_empty);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatProjectUpdateRecord {
    pub title: Option<String>,
    pub emoji: Option<String>,
    pub description: Option<String>,
    pub updated_at: String,
}

impl ChatProjectUpdateRecord {
    pub fn new(updated_at: impl Into<String>) -> Self {
        Self {
            title: None,
            emoji: None,
            description: None,
            updated_at: updated_at.into(),
        }
    }

    pub fn with_title(mut self, title: Option<String>) -> Self {
        self.title = title.and_then(non_empty);
        self
    }

    pub fn with_emoji(mut self, emoji: Option<String>) -> Self {
        self.emoji = emoji.and_then(non_empty);
        self
    }

    pub fn with_description(mut self, description: Option<String>) -> Self {
        self.description = description.and_then(non_empty);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformSessionRecord {
    pub session_id: String,
    pub platform_id: String,
    pub creator: String,
    pub display_name: Option<String>,
    pub is_group: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl PlatformSessionRecord {
    pub fn new(
        session_id: impl Into<String>,
        platform_id: impl Into<String>,
        creator: impl Into<String>,
        created_at: impl Into<String>,
    ) -> Self {
        let created_at = created_at.into();
        Self {
            session_id: session_id.into(),
            platform_id: platform_id.into(),
            creator: creator.into(),
            display_name: None,
            is_group: false,
            updated_at: created_at.clone(),
            created_at,
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = non_empty(display_name.into());
        self
    }

    pub fn group(mut self) -> Self {
        self.is_group = true;
        self
    }

    pub fn with_updated_at(mut self, updated_at: impl Into<String>) -> Self {
        self.updated_at = updated_at.into();
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionProjectMembershipRecord {
    pub session_id: String,
    pub project_id: String,
}

impl SessionProjectMembershipRecord {
    pub fn new(session_id: impl Into<String>, project_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            project_id: project_id.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatUiProjectRecord {
    pub project_id: String,
    pub creator: String,
    pub title: String,
    pub emoji: Option<String>,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl ChatUiProjectRecord {
    pub fn new(
        project_id: impl Into<String>,
        creator: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            creator: creator.into(),
            title: title.into(),
            emoji: Some(DEFAULT_CHAT_UI_PROJECT_EMOJI.to_string()),
            description: None,
            created_at: None,
            updated_at: None,
        }
    }

    pub fn with_emoji(mut self, emoji: impl Into<String>) -> Self {
        self.emoji = non_empty(emoji.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = non_empty(description.into());
        self
    }

    pub fn with_created_at(mut self, created_at: impl Into<String>) -> Self {
        self.created_at = non_empty(created_at.into());
        self
    }

    pub fn with_updated_at(mut self, updated_at: impl Into<String>) -> Self {
        self.updated_at = non_empty(updated_at.into());
        self
    }
}

impl From<ChatProjectRecord> for ChatUiProjectRecord {
    fn from(record: ChatProjectRecord) -> Self {
        Self {
            project_id: record.project_id,
            creator: record.creator,
            title: record.title,
            emoji: record.emoji,
            description: record.description,
            created_at: Some(record.created_at),
            updated_at: Some(record.updated_at),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatUiSessionRecord {
    pub session_id: String,
    pub platform_id: String,
    pub creator: String,
    pub display_name: Option<String>,
    pub is_group: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl ChatUiSessionRecord {
    pub fn new(
        session_id: impl Into<String>,
        platform_id: impl Into<String>,
        creator: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            platform_id: platform_id.into(),
            creator: creator.into(),
            display_name: None,
            is_group: false,
            created_at: None,
            updated_at: None,
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = non_empty(display_name.into());
        self
    }

    pub fn group(mut self) -> Self {
        self.is_group = true;
        self
    }

    pub fn with_created_at(mut self, created_at: impl Into<String>) -> Self {
        self.created_at = non_empty(created_at.into());
        self
    }

    pub fn with_updated_at(mut self, updated_at: impl Into<String>) -> Self {
        self.updated_at = non_empty(updated_at.into());
        self
    }
}

impl From<PlatformSessionRecord> for ChatUiSessionRecord {
    fn from(record: PlatformSessionRecord) -> Self {
        Self {
            session_id: record.session_id,
            platform_id: record.platform_id,
            creator: record.creator,
            display_name: record.display_name,
            is_group: record.is_group,
            created_at: Some(record.created_at),
            updated_at: Some(record.updated_at),
        }
    }
}

#[async_trait]
pub trait ChatProjectRepository: Send + Sync {
    async fn create_project(&self, record: ChatProjectCreateRecord) -> Result<ChatProjectRecord>;

    async fn project_by_id(&self, project_id: &str) -> Result<Option<ChatProjectRecord>>;

    async fn projects_by_creator(&self, creator: &str) -> Result<Vec<ChatProjectRecord>>;

    async fn update_project(
        &self,
        project_id: &str,
        record: ChatProjectUpdateRecord,
    ) -> Result<bool>;

    async fn delete_project(&self, project_id: &str) -> Result<bool>;

    async fn upsert_platform_session(&self, record: PlatformSessionRecord) -> Result<()>;

    async fn platform_session(&self, session_id: &str) -> Result<Option<PlatformSessionRecord>>;

    async fn add_session_to_project(
        &self,
        session_id: &str,
        project_id: &str,
    ) -> Result<SessionProjectMembershipRecord>;

    async fn remove_session_from_project(&self, session_id: &str) -> Result<bool>;

    async fn project_sessions(&self, project_id: &str) -> Result<Vec<PlatformSessionRecord>>;

    async fn project_by_session(
        &self,
        session_id: &str,
        creator: &str,
    ) -> Result<Option<ChatProjectRecord>>;
}

#[async_trait]
pub trait ChatUiProjectRepository: Send + Sync {
    async fn create_chatui_project(
        &self,
        record: ChatUiProjectRecord,
    ) -> Result<ChatUiProjectRecord>;

    async fn chatui_project(&self, project_id: &str) -> Result<Option<ChatUiProjectRecord>>;

    async fn chatui_projects_by_creator(&self, creator: &str) -> Result<Vec<ChatUiProjectRecord>>;

    async fn update_chatui_project(
        &self,
        record: ChatUiProjectRecord,
    ) -> Result<Option<ChatUiProjectRecord>>;

    async fn delete_chatui_project(&self, project_id: &str) -> Result<bool>;

    async fn upsert_chatui_session(&self, record: ChatUiSessionRecord) -> Result<()>;

    async fn chatui_session(&self, session_id: &str) -> Result<Option<ChatUiSessionRecord>>;

    async fn assign_session_to_project(&self, session_id: &str, project_id: &str) -> Result<()>;

    async fn remove_session_from_project(&self, session_id: &str) -> Result<bool>;

    async fn project_sessions(&self, project_id: &str) -> Result<Vec<ChatUiSessionRecord>>;

    async fn project_by_session(
        &self,
        session_id: &str,
        creator: &str,
    ) -> Result<Option<ChatUiProjectRecord>>;
}

pub type InMemoryChatUiProjectRepository = InMemoryChatProjectRepository;

#[derive(Default)]
pub struct InMemoryChatProjectRepository {
    next_project_id: RwLock<u64>,
    projects: RwLock<HashMap<String, ChatProjectRecord>>,
    sessions: RwLock<HashMap<String, PlatformSessionRecord>>,
    memberships: RwLock<HashMap<String, String>>,
}

impl InMemoryChatProjectRepository {
    pub fn new() -> Self {
        Self::default()
    }

    fn allocate_project_id(&self) -> Result<String> {
        let mut next = self
            .next_project_id
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("chat project id lock: {err}")))?;
        *next += 1;
        Ok(format!("project-{next}"))
    }
}

#[async_trait]
impl ChatProjectRepository for InMemoryChatProjectRepository {
    async fn create_project(&self, record: ChatProjectCreateRecord) -> Result<ChatProjectRecord> {
        let project_id = self.allocate_project_id()?;
        let project = ChatProjectRecord::new(project_id, record.creator, record.title, record.now)
            .with_emoji(record.emoji)
            .with_description(record.description);
        validate_project(&project)?;
        self.projects
            .write()
            .map_err(project_lock_error)?
            .insert(project.project_id.clone(), project.clone());
        Ok(project)
    }

    async fn project_by_id(&self, project_id: &str) -> Result<Option<ChatProjectRecord>> {
        let project_id = required(project_id, "project_id")?;
        Ok(self
            .projects
            .read()
            .map_err(project_lock_error)?
            .get(project_id)
            .cloned())
    }

    async fn projects_by_creator(&self, creator: &str) -> Result<Vec<ChatProjectRecord>> {
        let creator = required(creator, "creator")?;
        let mut projects = self
            .projects
            .read()
            .map_err(project_lock_error)?
            .values()
            .filter(|project| project.creator == creator)
            .cloned()
            .collect::<Vec<_>>();
        projects.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.project_id.cmp(&right.project_id))
        });
        Ok(projects)
    }

    async fn update_project(
        &self,
        project_id: &str,
        record: ChatProjectUpdateRecord,
    ) -> Result<bool> {
        let project_id = required(project_id, "project_id")?;
        let mut projects = self.projects.write().map_err(project_lock_error)?;
        let Some(project) = projects.get_mut(project_id) else {
            return Ok(false);
        };
        if let Some(title) = record.title {
            project.title = title;
        }
        if let Some(emoji) = record.emoji {
            project.emoji = Some(emoji);
        }
        if let Some(description) = record.description {
            project.description = Some(description);
        }
        project.updated_at = record.updated_at;
        validate_project(project)?;
        Ok(true)
    }

    async fn delete_project(&self, project_id: &str) -> Result<bool> {
        let project_id = required(project_id, "project_id")?;
        let removed = self
            .projects
            .write()
            .map_err(project_lock_error)?
            .remove(project_id)
            .is_some();
        if removed {
            self.memberships
                .write()
                .map_err(membership_lock_error)?
                .retain(|_, current_project_id| current_project_id != project_id);
        }
        Ok(removed)
    }

    async fn upsert_platform_session(&self, record: PlatformSessionRecord) -> Result<()> {
        validate_session(&record)?;
        self.sessions
            .write()
            .map_err(session_lock_error)?
            .insert(record.session_id.clone(), record);
        Ok(())
    }

    async fn platform_session(&self, session_id: &str) -> Result<Option<PlatformSessionRecord>> {
        let session_id = required(session_id, "session_id")?;
        Ok(self
            .sessions
            .read()
            .map_err(session_lock_error)?
            .get(session_id)
            .cloned())
    }

    async fn add_session_to_project(
        &self,
        session_id: &str,
        project_id: &str,
    ) -> Result<SessionProjectMembershipRecord> {
        let session_id = required(session_id, "session_id")?;
        let project_id = required(project_id, "project_id")?;
        self.memberships
            .write()
            .map_err(membership_lock_error)?
            .insert(session_id.to_string(), project_id.to_string());
        Ok(SessionProjectMembershipRecord::new(session_id, project_id))
    }

    async fn remove_session_from_project(&self, session_id: &str) -> Result<bool> {
        let session_id = required(session_id, "session_id")?;
        Ok(self
            .memberships
            .write()
            .map_err(membership_lock_error)?
            .remove(session_id)
            .is_some())
    }

    async fn project_sessions(&self, project_id: &str) -> Result<Vec<PlatformSessionRecord>> {
        let project_id = required(project_id, "project_id")?;
        let session_ids = self
            .memberships
            .read()
            .map_err(membership_lock_error)?
            .iter()
            .filter(|(_, current_project_id)| *current_project_id == project_id)
            .map(|(session_id, _)| session_id.clone())
            .collect::<Vec<_>>();
        let sessions = self.sessions.read().map_err(session_lock_error)?;
        let mut records = session_ids
            .iter()
            .filter_map(|session_id| sessions.get(session_id))
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.session_id.cmp(&right.session_id))
        });
        Ok(records)
    }

    async fn project_by_session(
        &self,
        session_id: &str,
        creator: &str,
    ) -> Result<Option<ChatProjectRecord>> {
        let session_id = required(session_id, "session_id")?;
        let creator = required(creator, "creator")?;
        let project_id = self
            .memberships
            .read()
            .map_err(membership_lock_error)?
            .get(session_id)
            .cloned();
        let Some(project_id) = project_id else {
            return Ok(None);
        };
        Ok(self
            .projects
            .read()
            .map_err(project_lock_error)?
            .get(&project_id)
            .filter(|project| project.creator == creator)
            .cloned())
    }
}

#[async_trait]
impl ChatUiProjectRepository for InMemoryChatProjectRepository {
    async fn create_chatui_project(
        &self,
        record: ChatUiProjectRecord,
    ) -> Result<ChatUiProjectRecord> {
        validate_chatui_project(&record)?;
        let project = chatui_project_to_project(record.clone());
        self.projects
            .write()
            .map_err(project_lock_error)?
            .insert(project.project_id.clone(), project);
        Ok(record)
    }

    async fn chatui_project(&self, project_id: &str) -> Result<Option<ChatUiProjectRecord>> {
        self.project_by_id(project_id)
            .await
            .map(|project| project.map(Into::into))
    }

    async fn chatui_projects_by_creator(&self, creator: &str) -> Result<Vec<ChatUiProjectRecord>> {
        self.projects_by_creator(creator)
            .await
            .map(|projects| projects.into_iter().map(Into::into).collect())
    }

    async fn update_chatui_project(
        &self,
        record: ChatUiProjectRecord,
    ) -> Result<Option<ChatUiProjectRecord>> {
        validate_chatui_project(&record)?;
        let project = chatui_project_to_project(record.clone());
        let mut projects = self.projects.write().map_err(project_lock_error)?;
        if !projects.contains_key(&project.project_id) {
            return Ok(None);
        }
        projects.insert(project.project_id.clone(), project);
        Ok(Some(record))
    }

    async fn delete_chatui_project(&self, project_id: &str) -> Result<bool> {
        self.delete_project(project_id).await
    }

    async fn upsert_chatui_session(&self, record: ChatUiSessionRecord) -> Result<()> {
        let session = chatui_session_to_platform_session(record);
        self.upsert_platform_session(session).await
    }

    async fn chatui_session(&self, session_id: &str) -> Result<Option<ChatUiSessionRecord>> {
        self.platform_session(session_id)
            .await
            .map(|session| session.map(Into::into))
    }

    async fn assign_session_to_project(&self, session_id: &str, project_id: &str) -> Result<()> {
        self.add_session_to_project(session_id, project_id)
            .await
            .map(|_| ())
    }

    async fn remove_session_from_project(&self, session_id: &str) -> Result<bool> {
        ChatProjectRepository::remove_session_from_project(self, session_id).await
    }

    async fn project_sessions(&self, project_id: &str) -> Result<Vec<ChatUiSessionRecord>> {
        ChatProjectRepository::project_sessions(self, project_id)
            .await
            .map(|sessions| sessions.into_iter().map(Into::into).collect())
    }

    async fn project_by_session(
        &self,
        session_id: &str,
        creator: &str,
    ) -> Result<Option<ChatUiProjectRecord>> {
        ChatProjectRepository::project_by_session(self, session_id, creator)
            .await
            .map(|project| project.map(Into::into))
    }
}

fn validate_project(project: &ChatProjectRecord) -> Result<()> {
    required(&project.project_id, "project_id")?;
    required(&project.creator, "creator")?;
    required(&project.title, "title")?;
    required(&project.created_at, "created_at")?;
    required(&project.updated_at, "updated_at")?;
    Ok(())
}

fn validate_session(session: &PlatformSessionRecord) -> Result<()> {
    required(&session.session_id, "session_id")?;
    required(&session.platform_id, "platform_id")?;
    required(&session.creator, "creator")?;
    required(&session.created_at, "created_at")?;
    required(&session.updated_at, "updated_at")?;
    Ok(())
}

fn validate_chatui_project(project: &ChatUiProjectRecord) -> Result<()> {
    required(&project.project_id, "project_id")?;
    required(&project.creator, "creator")?;
    required(&project.title, "title")?;
    Ok(())
}

fn chatui_project_to_project(record: ChatUiProjectRecord) -> ChatProjectRecord {
    let created_at = record
        .created_at
        .clone()
        .or_else(|| record.updated_at.clone())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
    let updated_at = record
        .updated_at
        .clone()
        .unwrap_or_else(|| created_at.clone());
    ChatProjectRecord::new(record.project_id, record.creator, record.title, created_at)
        .with_emoji(record.emoji)
        .with_description(record.description)
        .with_updated_at(updated_at)
}

fn chatui_session_to_platform_session(record: ChatUiSessionRecord) -> PlatformSessionRecord {
    let created_at = record
        .created_at
        .clone()
        .or_else(|| record.updated_at.clone())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
    let updated_at = record
        .updated_at
        .clone()
        .unwrap_or_else(|| created_at.clone());
    let mut session = PlatformSessionRecord::new(
        record.session_id,
        record.platform_id,
        record.creator,
        created_at,
    )
    .with_updated_at(updated_at);
    session.display_name = record.display_name;
    session.is_group = record.is_group;
    session
}

fn required<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AstrbotError::Pipeline(format!(
            "chat project {field} is required"
        )));
    }
    Ok(value)
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn project_lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("chat project lock: {err}"))
}

fn session_lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("platform session lock: {err}"))
}

fn membership_lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("session project membership lock: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{
        ChatProjectCreateRecord, ChatProjectRepository, InMemoryChatProjectRepository,
        PlatformSessionRecord,
    };

    #[tokio::test]
    async fn chat_project_repository_lists_projects_by_creator_recent_first() {
        let repository = InMemoryChatProjectRepository::new();
        let older = repository
            .create_project(ChatProjectCreateRecord::new(
                "alice",
                "Older",
                "2026-05-16T00:00:00Z",
            ))
            .await
            .expect("older project should store");
        let newer = repository
            .create_project(ChatProjectCreateRecord::new(
                "alice",
                "Newer",
                "2026-05-17T00:00:00Z",
            ))
            .await
            .expect("newer project should store");
        repository
            .create_project(ChatProjectCreateRecord::new(
                "bob",
                "Bob",
                "2026-05-18T00:00:00Z",
            ))
            .await
            .expect("bob project should store");

        let projects = repository
            .projects_by_creator("alice")
            .await
            .expect("projects should list");

        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].project_id, newer.project_id);
        assert_eq!(projects[1].project_id, older.project_id);
    }

    #[tokio::test]
    async fn chat_project_repository_reassigns_session_and_cleans_deleted_project() {
        let repository = InMemoryChatProjectRepository::new();
        let project_a = repository
            .create_project(ChatProjectCreateRecord::new(
                "alice",
                "A",
                "2026-05-17T00:00:00Z",
            ))
            .await
            .expect("project should store");
        let project_b = repository
            .create_project(ChatProjectCreateRecord::new(
                "alice",
                "B",
                "2026-05-17T00:00:01Z",
            ))
            .await
            .expect("project should store");
        repository
            .upsert_platform_session(PlatformSessionRecord::new(
                "session-1",
                "webchat",
                "alice",
                "2026-05-17T00:00:02Z",
            ))
            .await
            .expect("session should store");

        repository
            .add_session_to_project("session-1", &project_a.project_id)
            .await
            .expect("session should assign");
        repository
            .add_session_to_project("session-1", &project_b.project_id)
            .await
            .expect("session should reassign");

        assert!(
            repository
                .project_sessions(&project_a.project_id)
                .await
                .expect("sessions should load")
                .is_empty()
        );
        assert_eq!(
            repository
                .project_by_session("session-1", "alice")
                .await
                .expect("project should load")
                .expect("project should exist")
                .project_id,
            project_b.project_id
        );

        assert!(
            repository
                .delete_project(&project_b.project_id)
                .await
                .expect("project should delete")
        );
        assert!(
            repository
                .project_by_session("session-1", "alice")
                .await
                .expect("project lookup should work")
                .is_none()
        );
    }
}
