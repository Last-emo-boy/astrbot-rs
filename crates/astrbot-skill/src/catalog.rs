use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDescriptor {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub path: String,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub source: SkillSource,
}

impl SkillDescriptor {
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            path: path.into(),
            active: true,
            source: SkillSource::Local,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_source(mut self, source: SkillSource) -> Self {
        self.source = source;
        self
    }

    pub fn inactive(mut self) -> Self {
        self.active = false;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSource {
    #[default]
    Local,
    Sandbox,
    Synced,
}

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

    pub fn is_enabled(&self, skill_name: &str) -> bool {
        !self.disabled_skills.contains(skill_name)
            && self
                .allowed_skills
                .as_ref()
                .is_none_or(|allowed| allowed.contains(skill_name))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillCatalog {
    skills: Vec<SkillDescriptor>,
}

impl SkillCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_skill(&mut self, skill: SkillDescriptor) {
        if let Some(existing) = self.skills.iter_mut().find(|item| item.name == skill.name) {
            *existing = skill;
        } else {
            self.skills.push(skill);
        }
        self.skills
            .sort_by(|left, right| left.name.cmp(&right.name));
    }

    pub fn remove_skill(&mut self, name: &str) {
        self.skills.retain(|skill| skill.name != name);
    }

    pub fn skills(&self) -> &[SkillDescriptor] {
        &self.skills
    }

    pub fn active_skills(&self, policy: &SkillActivationPolicy) -> Vec<SkillDescriptor> {
        self.skills
            .iter()
            .filter(|skill| skill.active && policy.is_enabled(&skill.name))
            .cloned()
            .collect()
    }
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
    use super::{SkillActivationPolicy, SkillCatalog, SkillDescriptor};

    #[test]
    fn skill_catalog_applies_persona_allowlist_and_disabled_policy() {
        let mut catalog = SkillCatalog::new();
        catalog.add_skill(SkillDescriptor::new("write", "/skills/write/SKILL.md"));
        catalog.add_skill(SkillDescriptor::new("draw", "/skills/draw/SKILL.md"));
        catalog.add_skill(SkillDescriptor::new("off", "/skills/off/SKILL.md").inactive());

        let active = catalog.active_skills(
            &SkillActivationPolicy::all_enabled()
                .allow_only(["write", "draw"])
                .disable("draw"),
        );

        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "write");
    }

    #[test]
    fn skill_catalog_replaces_existing_skill_descriptor() {
        let mut catalog = SkillCatalog::new();
        catalog.add_skill(SkillDescriptor::new("write", "/old/SKILL.md"));
        catalog
            .add_skill(SkillDescriptor::new("write", "/new/SKILL.md").with_description("new desc"));

        assert_eq!(catalog.skills().len(), 1);
        assert_eq!(catalog.skills()[0].path, "/new/SKILL.md");
        assert_eq!(catalog.skills()[0].description, "new desc");
    }
}
