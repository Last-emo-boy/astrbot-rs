use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{SkillDescriptor, SkillSource, is_valid_skill_name};

pub const SANDBOX_SKILLS_CACHE_VERSION: u32 = 1;
pub const SANDBOX_SKILLS_ROOT: &str = "skills";
pub const SANDBOX_WORKSPACE_ROOT: &str = "/workspace";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSandboxCache {
    #[serde(default = "default_cache_version")]
    pub version: u32,
    #[serde(default)]
    pub skills: Vec<SkillSandboxEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

impl Default for SkillSandboxCache {
    fn default() -> Self {
        Self {
            version: SANDBOX_SKILLS_CACHE_VERSION,
            skills: Vec::new(),
            updated_at: None,
        }
    }
}

impl SkillSandboxCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_entries<I>(entries: I) -> Self
    where
        I: IntoIterator<Item = SkillSandboxEntry>,
    {
        let mut deduped = BTreeMap::new();
        for entry in entries {
            if !is_valid_skill_name(&entry.name) {
                continue;
            }
            deduped.insert(entry.name.clone(), entry.normalized());
        }

        Self {
            skills: deduped.into_values().collect(),
            ..Self::default()
        }
    }

    pub fn status(&self, exists: bool) -> SkillSandboxCacheStatus {
        SkillSandboxCacheStatus {
            exists,
            ready: !self.skills.is_empty(),
            count: self.skills.len(),
            updated_at: self.updated_at.clone(),
        }
    }

    pub fn as_descriptors(&self) -> Vec<SkillDescriptor> {
        self.skills
            .iter()
            .map(|entry| {
                SkillDescriptor::new(entry.name.clone(), entry.effective_path())
                    .with_description(entry.description.clone())
                    .with_source(SkillSource::Sandbox)
            })
            .collect()
    }

    pub fn entry(&self, name: &str) -> Option<&SkillSandboxEntry> {
        self.skills.iter().find(|entry| entry.name == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSandboxEntry {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub path: String,
}

impl SkillSandboxEntry {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            path: default_sandbox_skill_path(&name),
            name,
            description: String::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    pub fn effective_path(&self) -> String {
        if self.path.trim().is_empty() {
            default_sandbox_skill_path(&self.name)
        } else {
            self.path.replace('\\', "/")
        }
    }

    fn normalized(mut self) -> Self {
        if self.path.trim().is_empty() {
            self.path = default_sandbox_skill_path(&self.name);
        } else {
            self.path = self.path.replace('\\', "/");
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSandboxCacheStatus {
    pub exists: bool,
    pub ready: bool,
    pub count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

pub fn default_sandbox_skill_path(name: &str) -> String {
    format!("{SANDBOX_WORKSPACE_ROOT}/{SANDBOX_SKILLS_ROOT}/{name}/SKILL.md")
}

fn default_cache_version() -> u32 {
    SANDBOX_SKILLS_CACHE_VERSION
}

#[cfg(test)]
mod tests {
    use crate::{SkillSandboxCache, SkillSandboxEntry};

    #[test]
    fn sandbox_cache_filters_invalid_names_and_dedupes_entries() {
        let cache = SkillSandboxCache::from_entries([
            SkillSandboxEntry::new("preset").with_description("old"),
            SkillSandboxEntry::new("bad name"),
            SkillSandboxEntry::new("preset").with_description("new"),
        ]);

        assert_eq!(cache.skills.len(), 1);
        assert_eq!(cache.skills[0].name, "preset");
        assert_eq!(cache.skills[0].description, "new");
        assert_eq!(cache.status(true).count, 1);
        assert!(cache.status(true).ready);
    }

    #[test]
    fn sandbox_cache_entries_become_sandbox_only_descriptors() {
        let cache = SkillSandboxCache::from_entries([SkillSandboxEntry::new("preset")]);
        let descriptors = cache.as_descriptors();

        assert_eq!(descriptors.len(), 1);
        assert!(descriptors[0].is_sandbox_only());
        assert_eq!(
            descriptors[0].path,
            "/workspace/skills/preset/SKILL.md".to_string()
        );
    }
}
