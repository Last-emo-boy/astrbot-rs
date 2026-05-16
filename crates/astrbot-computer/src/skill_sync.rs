use std::sync::{Arc, RwLock};

use astrbot_core::{AstrbotError, Result};
use astrbot_skill::{SkillDescriptor, SkillSource};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxSkill {
    pub name: String,
    pub description: String,
    pub path: String,
}

impl SandboxSkill {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            path: path.into(),
        }
    }
}

impl From<SandboxSkill> for SkillDescriptor {
    fn from(skill: SandboxSkill) -> Self {
        SkillDescriptor::new(skill.name, skill.path)
            .with_description(skill.description)
            .with_source(SkillSource::Sandbox)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxSkillBundle {
    pub source_root: String,
    pub archive_name: String,
    pub skill_dirs: Vec<String>,
}

impl SandboxSkillBundle {
    pub fn new(source_root: impl Into<String>, skill_dirs: Vec<String>) -> Self {
        Self {
            source_root: source_root.into(),
            archive_name: "skills_bundle.zip".to_string(),
            skill_dirs,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.skill_dirs.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SandboxSkillSyncStage {
    Upload,
    Apply,
    Scan,
    CacheRefresh,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxSkillSyncStep {
    pub stage: SandboxSkillSyncStage,
    pub skipped: bool,
    pub detail: String,
}

impl SandboxSkillSyncStep {
    pub fn completed(stage: SandboxSkillSyncStage, detail: impl Into<String>) -> Self {
        Self {
            stage,
            skipped: false,
            detail: detail.into(),
        }
    }

    pub fn skipped(stage: SandboxSkillSyncStage, detail: impl Into<String>) -> Self {
        Self {
            stage,
            skipped: true,
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SandboxSkillSyncPlan {
    pub steps: Vec<SandboxSkillSyncStep>,
    pub skills: Vec<SandboxSkill>,
    pub managed_skills: Vec<String>,
}

impl SandboxSkillSyncPlan {
    pub fn skill_descriptors(&self) -> Vec<SkillDescriptor> {
        self.skills
            .iter()
            .cloned()
            .map(SkillDescriptor::from)
            .collect()
    }
}

#[async_trait]
pub trait SandboxSkillCache: Send + Sync {
    async fn replace(&self, skills: Vec<SandboxSkill>) -> Result<()>;

    async fn skills(&self) -> Result<Vec<SandboxSkill>>;
}

#[derive(Default)]
pub struct InMemorySandboxSkillCache {
    skills: RwLock<Vec<SandboxSkill>>,
}

impl InMemorySandboxSkillCache {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SandboxSkillCache for InMemorySandboxSkillCache {
    async fn replace(&self, skills: Vec<SandboxSkill>) -> Result<()> {
        *self.skills.write().map_err(lock_error)? = skills;
        Ok(())
    }

    async fn skills(&self) -> Result<Vec<SandboxSkill>> {
        Ok(self.skills.read().map_err(lock_error)?.clone())
    }
}

#[async_trait]
pub trait SandboxSkillSyncService: Send + Sync {
    async fn sync_bundle(&self, bundle: SandboxSkillBundle) -> Result<SandboxSkillSyncPlan>;
}

pub struct PlanningSandboxSkillSyncService {
    cache: Arc<dyn SandboxSkillCache>,
    sandbox_root: String,
}

impl PlanningSandboxSkillSyncService {
    pub fn new(cache: Arc<dyn SandboxSkillCache>) -> Self {
        Self {
            cache,
            sandbox_root: "/workspace/skills".to_string(),
        }
    }

    pub fn with_sandbox_root(mut self, sandbox_root: impl Into<String>) -> Self {
        let sandbox_root = sandbox_root.into();
        if !sandbox_root.trim().is_empty() {
            self.sandbox_root = sandbox_root;
        }
        self
    }
}

#[async_trait]
impl SandboxSkillSyncService for PlanningSandboxSkillSyncService {
    async fn sync_bundle(&self, bundle: SandboxSkillBundle) -> Result<SandboxSkillSyncPlan> {
        let mut steps = Vec::new();
        if bundle.is_empty() {
            steps.push(SandboxSkillSyncStep::skipped(
                SandboxSkillSyncStage::Upload,
                "no local skills",
            ));
        } else {
            steps.push(SandboxSkillSyncStep::completed(
                SandboxSkillSyncStage::Upload,
                bundle.archive_name.clone(),
            ));
        }
        steps.push(SandboxSkillSyncStep::completed(
            SandboxSkillSyncStage::Apply,
            "replace managed skills",
        ));

        let skills = bundle
            .skill_dirs
            .iter()
            .map(|name| {
                SandboxSkill::new(
                    name,
                    "",
                    format!(
                        "{}/{}/SKILL.md",
                        self.sandbox_root.trim_end_matches('/'),
                        name
                    ),
                )
            })
            .collect::<Vec<_>>();
        steps.push(SandboxSkillSyncStep::completed(
            SandboxSkillSyncStage::Scan,
            format!("{} skills", skills.len()),
        ));

        self.cache.replace(skills.clone()).await?;
        steps.push(SandboxSkillSyncStep::completed(
            SandboxSkillSyncStage::CacheRefresh,
            "sandbox skill cache updated",
        ));

        Ok(SandboxSkillSyncPlan {
            steps,
            skills,
            managed_skills: bundle.skill_dirs,
        })
    }
}

fn lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("sandbox skill cache lock: {err}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use astrbot_skill::SkillSource;

    use super::{
        InMemorySandboxSkillCache, PlanningSandboxSkillSyncService, SandboxSkillBundle,
        SandboxSkillCache, SandboxSkillSyncService, SandboxSkillSyncStage,
    };

    #[tokio::test]
    async fn skill_sync_plan_splits_upload_apply_scan_and_cache_refresh() {
        let cache = Arc::new(InMemorySandboxSkillCache::new());
        let service = PlanningSandboxSkillSyncService::new(cache.clone());

        let plan = service
            .sync_bundle(SandboxSkillBundle::new(
                "skills",
                vec!["writer".to_string(), "browser".to_string()],
            ))
            .await
            .expect("sync should plan");

        assert_eq!(
            plan.steps.iter().map(|step| step.stage).collect::<Vec<_>>(),
            [
                SandboxSkillSyncStage::Upload,
                SandboxSkillSyncStage::Apply,
                SandboxSkillSyncStage::Scan,
                SandboxSkillSyncStage::CacheRefresh
            ]
        );
        assert_eq!(cache.skills().await.expect("cache should read").len(), 2);
        assert!(
            plan.skill_descriptors()
                .iter()
                .all(|skill| skill.source == SkillSource::Sandbox)
        );
    }

    #[tokio::test]
    async fn empty_skill_bundle_skips_upload_but_refreshes_cache() {
        let cache = Arc::new(InMemorySandboxSkillCache::new());
        let service = PlanningSandboxSkillSyncService::new(cache.clone());

        let plan = service
            .sync_bundle(SandboxSkillBundle::new("skills", Vec::new()))
            .await
            .expect("sync should plan");

        assert!(plan.steps[0].skipped);
        assert!(cache.skills().await.expect("cache should read").is_empty());
    }
}
