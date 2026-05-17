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

    pub fn with_active(mut self, active: bool) -> Self {
        self.active = active;
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

    pub fn is_sandbox_only(&self) -> bool {
        self.source == SkillSource::Sandbox
    }

    pub fn local_exists(&self) -> bool {
        matches!(self.source, SkillSource::Local | SkillSource::Synced)
    }

    pub fn sandbox_exists(&self) -> bool {
        matches!(self.source, SkillSource::Sandbox | SkillSource::Synced)
    }

    pub fn source_type(&self) -> &'static str {
        match self.source {
            SkillSource::Local => "local_only",
            SkillSource::Sandbox => "sandbox_only",
            SkillSource::Synced => "both",
        }
    }

    pub fn source_label(&self) -> &'static str {
        match self.source {
            SkillSource::Local => "local",
            SkillSource::Sandbox => "sandbox_preset",
            SkillSource::Synced => "synced",
        }
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
pub struct SkillCatalog {
    skills: Vec<SkillDescriptor>,
}

impl SkillCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_skills<I>(skills: I) -> Self
    where
        I: IntoIterator<Item = SkillDescriptor>,
    {
        let mut catalog = Self::new();
        for skill in skills {
            catalog.add_skill(skill);
        }
        catalog
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

    pub fn skill(&self, name: &str) -> Option<&SkillDescriptor> {
        self.skills.iter().find(|skill| skill.name == name)
    }

    pub fn is_sandbox_only(&self, name: &str) -> bool {
        self.skill(name)
            .is_some_and(SkillDescriptor::is_sandbox_only)
    }

    pub fn skills(&self) -> &[SkillDescriptor] {
        &self.skills
    }

    pub fn active_skills(&self, policy: &crate::SkillActivationPolicy) -> Vec<SkillDescriptor> {
        self.skills
            .iter()
            .filter(|skill| skill.active && policy.is_enabled(&skill.name))
            .cloned()
            .collect()
    }
}

pub fn is_valid_skill_name(name: &str) -> bool {
    !name.is_empty()
        && !matches!(name, "." | "..")
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::{SkillCatalog, SkillDescriptor, SkillSource, is_valid_skill_name};
    use crate::SkillActivationPolicy;

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

    #[test]
    fn skill_descriptor_marks_sandbox_only_sources_explicitly() {
        let descriptor = SkillDescriptor::new("preset", "/workspace/skills/preset/SKILL.md")
            .with_source(SkillSource::Sandbox);

        assert!(descriptor.is_sandbox_only());
        assert!(!descriptor.local_exists());
        assert!(descriptor.sandbox_exists());
        assert_eq!(descriptor.source_type(), "sandbox_only");
    }

    #[test]
    fn skill_name_validation_matches_package_folder_policy() {
        assert!(is_valid_skill_name("writer.tool-1"));
        assert!(!is_valid_skill_name("../writer"));
        assert!(!is_valid_skill_name("bad name"));
        assert!(!is_valid_skill_name(".."));
    }
}
