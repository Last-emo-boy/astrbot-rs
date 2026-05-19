use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{SkillCatalog, SkillPackageError, is_valid_skill_name};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillActivationPolicy {
    allowed_skills: Option<BTreeSet<String>>,
    disabled_skills: BTreeSet<String>,
}

impl SkillActivationPolicy {
    pub fn all_enabled() -> Self {
        Self::default()
    }

    pub fn allow_only<I, S>(mut self, skills: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_skills = Some(non_empty_set(skills));
        self
    }

    pub fn disable(mut self, skill_name: impl Into<String>) -> Self {
        let skill_name = skill_name.into();
        if !skill_name.trim().is_empty() {
            self.disabled_skills.insert(skill_name);
        }
        self
    }

    pub fn and(mut self, other: &Self) -> Self {
        self.disabled_skills
            .extend(other.disabled_skills.iter().cloned());
        self.allowed_skills = match (self.allowed_skills.take(), other.allowed_skills.as_ref()) {
            (Some(left), Some(right)) => Some(left.intersection(right).cloned().collect()),
            (Some(left), None) => Some(left),
            (None, Some(right)) => Some(right.clone()),
            (None, None) => None,
        };
        self
    }

    pub fn is_enabled(&self, skill_name: &str) -> bool {
        !self.disabled_skills.contains(skill_name)
            && self
                .allowed_skills
                .as_ref()
                .is_none_or(|allowed| allowed.contains(skill_name))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillActivationConfig {
    #[serde(default)]
    pub skills: BTreeMap<String, SkillActivationState>,
}

impl SkillActivationConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self, skill_name: &str) -> bool {
        self.skills
            .get(skill_name)
            .map(|state| state.active)
            .unwrap_or(true)
    }

    pub fn set_active(
        &mut self,
        catalog: &SkillCatalog,
        skill_name: impl Into<String>,
        active: bool,
    ) -> Result<SkillActivationChange, SkillPackageError> {
        let skill_name = skill_name.into();
        if !is_valid_skill_name(&skill_name) {
            return Err(SkillPackageError::invalid_skill_name(skill_name));
        }
        if catalog.skill(&skill_name).is_none() {
            return Err(SkillPackageError::SkillNotFound { name: skill_name });
        }
        if catalog.is_sandbox_only(&skill_name) {
            return Err(SkillPackageError::sandbox_only_mutation(
                skill_name,
                SkillPackageOperation::Activation,
            ));
        }

        self.skills
            .insert(skill_name.clone(), SkillActivationState { active });
        Ok(SkillActivationChange { skill_name, active })
    }

    pub fn remove(&mut self, skill_name: &str) {
        self.skills.remove(skill_name);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillActivationState {
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillActivationChange {
    pub skill_name: String,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillPackageOperation {
    Activation,
    Install,
    Delete,
}

fn non_empty_set<I, S>(items: I) -> BTreeSet<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    items
        .into_iter()
        .map(Into::into)
        .filter(|item| !item.trim().is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{
        SkillActivationConfig, SkillCatalog, SkillDescriptor, SkillPackageError, SkillSource,
    };

    #[test]
    fn activation_config_rejects_sandbox_only_mutation() {
        let catalog = SkillCatalog::from_skills([SkillDescriptor::new(
            "preset",
            "/workspace/skills/preset/SKILL.md",
        )
        .with_source(SkillSource::Sandbox)]);
        let mut config = SkillActivationConfig::new();

        let error = config
            .set_active(&catalog, "preset", false)
            .expect_err("sandbox-only skill should not be mutable locally");

        assert!(matches!(
            error,
            SkillPackageError::SandboxOnlyMutation { .. }
        ));
    }

    #[test]
    fn activation_config_defaults_to_active_and_records_overrides() {
        let catalog =
            SkillCatalog::from_skills([SkillDescriptor::new("writer", "/skills/writer/SKILL.md")]);
        let mut config = SkillActivationConfig::new();

        assert!(config.is_active("writer"));
        let change = config
            .set_active(&catalog, "writer", false)
            .expect("local skill should update");

        assert_eq!(change.skill_name, "writer");
        assert!(!change.active);
        assert!(!config.is_active("writer"));
    }

    #[test]
    fn activation_config_rejects_missing_skill_mutation() {
        let catalog = SkillCatalog::new();
        let mut config = SkillActivationConfig::new();

        let error = config
            .set_active(&catalog, "missing", false)
            .expect_err("missing skill should not be mutable");

        assert!(matches!(error, SkillPackageError::SkillNotFound { .. }));
    }

    #[test]
    fn activation_policy_and_intersects_allowlist_and_disabled_names() {
        let policy = super::SkillActivationPolicy::all_enabled()
            .allow_only(["writer", "draw"])
            .disable("draw")
            .and(&super::SkillActivationPolicy::all_enabled().allow_only(["writer", "preset"]));

        assert!(policy.is_enabled("writer"));
        assert!(!policy.is_enabled("draw"));
        assert!(!policy.is_enabled("preset"));
    }
}
