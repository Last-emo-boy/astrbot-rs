use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};
use astrbot_storage::{
    ChatProjectCreateRecord, ChatProjectRecord, ChatProjectRepository, ChatProjectUpdateRecord,
    PlatformSessionRecord, SessionProjectMembershipRecord,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatProjectDraft {
    pub creator: String,
    pub title: String,
    pub emoji: Option<String>,
    pub description: Option<String>,
}

impl ChatProjectDraft {
    pub fn new(creator: impl Into<String>, title: impl Into<String>) -> Result<Self> {
        let creator = normalize_required("creator", creator.into())?;
        let title = normalize_required("title", title.into())?;
        Ok(Self {
            creator,
            title,
            emoji: None,
            description: None,
        })
    }

    pub fn with_emoji(mut self, emoji: impl Into<String>) -> Self {
        self.emoji = non_empty(emoji.into());
        self
    }

    pub fn with_optional_emoji(mut self, emoji: Option<String>) -> Self {
        self.emoji = emoji.and_then(non_empty);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = non_empty(description.into());
        self
    }

    pub fn with_optional_description(mut self, description: Option<String>) -> Self {
        self.description = description.and_then(non_empty);
        self
    }

    fn into_create_record(self, now: String) -> ChatProjectCreateRecord {
        ChatProjectCreateRecord::new(self.creator, self.title, now)
            .with_emoji(self.emoji)
            .with_description(self.description)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatProjectPatch {
    pub title: Option<String>,
    pub emoji: Option<String>,
    pub description: Option<String>,
}

impl ChatProjectPatch {
    pub fn new() -> Self {
        Self::default()
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

    fn into_update_record(self, now: String) -> ChatProjectUpdateRecord {
        ChatProjectUpdateRecord::new(now)
            .with_title(self.title)
            .with_emoji(self.emoji)
            .with_description(self.description)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatProjectOwnershipDecision {
    Allowed,
    NotFound,
    Denied,
}

impl ChatProjectOwnershipDecision {
    pub fn ensure_allowed(self, resource: &str) -> Result<()> {
        match self {
            Self::Allowed => Ok(()),
            Self::NotFound => Err(AstrbotError::Pipeline(format!("{resource} not found"))),
            Self::Denied => Err(AstrbotError::Pipeline("permission denied".to_string())),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChatProjectOwnershipPolicy;

impl ChatProjectOwnershipPolicy {
    pub fn project_access(
        &self,
        project: Option<&ChatProjectRecord>,
        actor: &str,
    ) -> ChatProjectOwnershipDecision {
        match project {
            None => ChatProjectOwnershipDecision::NotFound,
            Some(project) if project.creator == actor => ChatProjectOwnershipDecision::Allowed,
            Some(_) => ChatProjectOwnershipDecision::Denied,
        }
    }

    pub fn session_access(
        &self,
        session: Option<&PlatformSessionRecord>,
        actor: &str,
    ) -> ChatProjectOwnershipDecision {
        match session {
            None => ChatProjectOwnershipDecision::NotFound,
            Some(session) if session.creator == actor => ChatProjectOwnershipDecision::Allowed,
            Some(_) => ChatProjectOwnershipDecision::Denied,
        }
    }

    pub fn can_add_session(
        &self,
        project: Option<&ChatProjectRecord>,
        session: Option<&PlatformSessionRecord>,
        actor: &str,
    ) -> ChatProjectOwnershipDecision {
        let project_access = self.project_access(project, actor);
        if project_access != ChatProjectOwnershipDecision::Allowed {
            return project_access;
        }
        self.session_access(session, actor)
    }
}

#[derive(Clone)]
pub struct ChatProjectService {
    repository: Arc<dyn ChatProjectRepository>,
    ownership: ChatProjectOwnershipPolicy,
}

impl ChatProjectService {
    pub fn new(repository: Arc<dyn ChatProjectRepository>) -> Self {
        Self {
            repository,
            ownership: ChatProjectOwnershipPolicy,
        }
    }

    pub fn repository(&self) -> Arc<dyn ChatProjectRepository> {
        self.repository.clone()
    }

    pub async fn create_project(
        &self,
        draft: ChatProjectDraft,
        now: impl Into<String>,
    ) -> Result<ChatProjectRecord> {
        self.repository
            .create_project(draft.into_create_record(now.into()))
            .await
    }

    pub async fn list_projects(&self, actor: &str) -> Result<Vec<ChatProjectRecord>> {
        self.repository.projects_by_creator(actor).await
    }

    pub async fn get_project(&self, actor: &str, project_id: &str) -> Result<ChatProjectRecord> {
        let project = self.repository.project_by_id(project_id).await?;
        self.ownership
            .project_access(project.as_ref(), actor)
            .ensure_allowed("project")?;
        Ok(project.expect("allowed ownership decision requires project"))
    }

    pub async fn update_project(
        &self,
        actor: &str,
        project_id: &str,
        patch: ChatProjectPatch,
        now: impl Into<String>,
    ) -> Result<()> {
        let project = self.repository.project_by_id(project_id).await?;
        self.ownership
            .project_access(project.as_ref(), actor)
            .ensure_allowed("project")?;
        self.repository
            .update_project(project_id, patch.into_update_record(now.into()))
            .await?;
        Ok(())
    }

    pub async fn delete_project(&self, actor: &str, project_id: &str) -> Result<()> {
        let project = self.repository.project_by_id(project_id).await?;
        self.ownership
            .project_access(project.as_ref(), actor)
            .ensure_allowed("project")?;
        self.repository.delete_project(project_id).await?;
        Ok(())
    }

    pub async fn add_session_to_project(
        &self,
        actor: &str,
        session_id: &str,
        project_id: &str,
    ) -> Result<SessionProjectMembershipRecord> {
        let project = self.repository.project_by_id(project_id).await?;
        let session = self.repository.platform_session(session_id).await?;
        self.ownership
            .can_add_session(project.as_ref(), session.as_ref(), actor)
            .ensure_allowed("project or session")?;
        self.repository
            .add_session_to_project(session_id, project_id)
            .await
    }

    pub async fn remove_session_from_project(&self, actor: &str, session_id: &str) -> Result<()> {
        let session = self.repository.platform_session(session_id).await?;
        self.ownership
            .session_access(session.as_ref(), actor)
            .ensure_allowed("session")?;
        self.repository
            .remove_session_from_project(session_id)
            .await?;
        Ok(())
    }

    pub async fn project_sessions(
        &self,
        actor: &str,
        project_id: &str,
    ) -> Result<Vec<PlatformSessionRecord>> {
        let project = self.repository.project_by_id(project_id).await?;
        self.ownership
            .project_access(project.as_ref(), actor)
            .ensure_allowed("project")?;
        self.repository.project_sessions(project_id).await
    }
}

fn normalize_required(field: &str, value: String) -> Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(AstrbotError::Pipeline(format!("{field} is required")));
    }
    Ok(value)
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use astrbot_storage::{
        ChatProjectRepository, InMemoryChatProjectRepository, PlatformSessionRecord,
    };

    use super::{ChatProjectDraft, ChatProjectService};

    #[tokio::test]
    async fn project_service_enforces_creator_ownership_for_projects_and_sessions() {
        let repository = Arc::new(InMemoryChatProjectRepository::new());
        repository
            .upsert_platform_session(PlatformSessionRecord::new(
                "session-1",
                "webchat",
                "alice",
                "2026-05-17T00:00:01Z",
            ))
            .await
            .expect("session should store");
        repository
            .upsert_platform_session(PlatformSessionRecord::new(
                "session-2",
                "webchat",
                "bob",
                "2026-05-17T00:00:02Z",
            ))
            .await
            .expect("session should store");
        let service = ChatProjectService::new(repository);
        let project = service
            .create_project(
                ChatProjectDraft::new("alice", "Research").expect("draft should build"),
                "2026-05-17T00:00:00Z",
            )
            .await
            .expect("project should create");

        service
            .add_session_to_project("alice", "session-1", &project.project_id)
            .await
            .expect("alice should add her session");

        assert!(
            service
                .add_session_to_project("alice", "session-2", &project.project_id)
                .await
                .is_err()
        );
        assert!(
            service
                .get_project("bob", &project.project_id)
                .await
                .is_err()
        );
    }
}
