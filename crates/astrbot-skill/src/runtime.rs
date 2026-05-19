use std::collections::BTreeSet;

use crate::{
    SkillActivationChange, SkillActivationConfig, SkillActivationPolicy, SkillCatalog,
    SkillDescriptor, SkillPackageDeletePlan, SkillPackageError, SkillPackageInstallPlan,
    SkillPromptInventory, SkillSandboxCache, SkillSource,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillRuntimeSnapshot {
    pub catalog: SkillCatalog,
    pub activation: SkillActivationConfig,
    pub sandbox_cache: Option<SkillSandboxCache>,
    pub sandbox_cache_exists: bool,
}

impl SkillRuntimeSnapshot {
    pub fn new(catalog: SkillCatalog) -> Self {
        Self {
            catalog,
            activation: SkillActivationConfig::new(),
            sandbox_cache: None,
            sandbox_cache_exists: false,
        }
    }

    pub fn with_activation_config(mut self, activation: SkillActivationConfig) -> Self {
        self.activation = activation;
        self
    }

    pub fn with_sandbox_cache(mut self, sandbox_cache: SkillSandboxCache, exists: bool) -> Self {
        self.sandbox_cache = Some(sandbox_cache);
        self.sandbox_cache_exists = exists;
        self
    }

    pub fn catalog_with_sandbox(&self) -> SkillCatalog {
        let mut catalog = self.catalog.clone();
        let Some(cache) = self.sandbox_cache.as_ref() else {
            return catalog;
        };

        for sandbox_skill in cache.as_descriptors() {
            if let Some(existing) = catalog.skill(&sandbox_skill.name).cloned() {
                catalog.add_skill(existing.with_source(SkillSource::Synced));
            } else {
                catalog.add_skill(sandbox_skill);
            }
        }

        catalog
    }

    pub fn active_catalog(&self) -> SkillCatalog {
        let mut catalog = self.catalog_with_sandbox();
        for skill in catalog.skills().to_vec() {
            let active = skill.active && self.activation.is_active(&skill.name);
            catalog.add_skill(skill.with_active(active));
        }
        catalog
    }

    pub fn activation_policy(&self) -> SkillActivationPolicy {
        self.active_catalog()
            .skills()
            .iter()
            .filter(|skill| !skill.active)
            .fold(SkillActivationPolicy::all_enabled(), |policy, skill| {
                policy.disable(skill.name.clone())
            })
    }

    pub fn prompt_inventory(&self, persona_policy: &SkillActivationPolicy) -> SkillPromptInventory {
        let policy = merge_activation_policies(self.activation_policy(), persona_policy);
        SkillPromptInventory::from_catalog(&self.catalog_with_sandbox(), &policy)
    }

    pub fn set_active(
        &mut self,
        skill_name: impl Into<String>,
        active: bool,
    ) -> Result<SkillActivationChange, SkillPackageError> {
        let catalog = self.catalog_with_sandbox();
        self.activation.set_active(&catalog, skill_name, active)
    }

    pub fn install_package(
        &mut self,
        request: SkillRuntimeInstallRequest,
    ) -> Result<SkillRuntimeInstallOutcome, SkillPackageError> {
        let plan = SkillPackageInstallPlan::from_zip_entries(request.entries, request.overwrite)?;
        if !plan.overwrite && self.catalog.skill(&plan.skill_name).is_some() {
            return Err(SkillPackageError::SkillAlreadyExists {
                name: plan.skill_name,
            });
        }

        let descriptor = SkillDescriptor::new(
            plan.skill_name.clone(),
            request
                .manifest_path
                .unwrap_or_else(|| format!("skills/{}/SKILL.md", plan.skill_name)),
        )
        .with_description(
            request
                .description
                .unwrap_or_else(|| "Installed from dashboard upload".to_string()),
        )
        .with_active(true)
        .with_source(SkillSource::Local);
        self.catalog.add_skill(descriptor.clone());
        self.activation.remove(&plan.skill_name);
        self.sync_sandbox_cache_from_local_catalog();

        Ok(SkillRuntimeInstallOutcome {
            plan,
            skill: descriptor,
            sandbox_cache: self.sandbox_cache.clone(),
        })
    }

    pub fn delete_package(
        &mut self,
        skill_name: impl Into<String>,
    ) -> Result<SkillRuntimeDeleteOutcome, SkillPackageError> {
        let skill_name = skill_name.into();
        let plan = SkillPackageDeletePlan::from_catalog(&self.catalog_with_sandbox(), &skill_name)?;
        self.catalog.remove_skill(&plan.skill_name);
        self.activation.remove(&plan.skill_name);
        if plan.remove_sandbox_cache_entry {
            self.sync_sandbox_cache_from_local_catalog_excluding([plan.skill_name.clone()]);
        }
        let remaining_skill = self.catalog_with_sandbox().skill(&plan.skill_name).cloned();

        Ok(SkillRuntimeDeleteOutcome {
            plan,
            deleted: true,
            remaining_skill,
            sandbox_cache: self.sandbox_cache.clone(),
        })
    }

    pub fn sync_sandbox_cache_from_local_catalog(&mut self) -> SkillSandboxCache {
        self.sync_sandbox_cache_from_local_catalog_excluding(Vec::<String>::new())
    }

    fn sync_sandbox_cache_from_local_catalog_excluding<I, S>(
        &mut self,
        removed_managed_names: I,
    ) -> SkillSandboxCache
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let local_names = self
            .catalog
            .skills()
            .iter()
            .filter(|skill| skill.local_exists())
            .map(|skill| skill.name.clone())
            .collect::<BTreeSet<_>>();
        let removed_managed_names = removed_managed_names
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        let preserved = self
            .sandbox_cache
            .as_ref()
            .into_iter()
            .flat_map(|cache| cache.skills.iter())
            .filter(|entry| {
                !local_names.contains(&entry.name) && !removed_managed_names.contains(&entry.name)
            })
            .cloned();
        let local = self
            .catalog
            .skills()
            .iter()
            .filter(|skill| skill.local_exists())
            .map(|skill| {
                crate::SkillSandboxEntry::new(skill.name.clone())
                    .with_description(skill.description.clone())
            });
        let cache = SkillSandboxCache::from_entries(preserved.chain(local));
        self.sandbox_cache = Some(cache.clone());
        self.sandbox_cache_exists = true;
        cache
    }

    pub fn replace_sandbox_cache(&mut self, cache: SkillSandboxCache, exists: bool) {
        self.sandbox_cache = Some(cache);
        self.sandbox_cache_exists = exists;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillRuntimeInstallRequest {
    pub entries: Vec<String>,
    pub overwrite: bool,
    pub description: Option<String>,
    pub manifest_path: Option<String>,
}

impl SkillRuntimeInstallRequest {
    pub fn new(entries: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            entries: entries.into_iter().map(Into::into).collect(),
            overwrite: true,
            description: None,
            manifest_path: None,
        }
    }

    pub fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_manifest_path(mut self, manifest_path: impl Into<String>) -> Self {
        self.manifest_path = Some(manifest_path.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillRuntimeInstallOutcome {
    pub plan: SkillPackageInstallPlan,
    pub skill: SkillDescriptor,
    pub sandbox_cache: Option<SkillSandboxCache>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillRuntimeDeleteOutcome {
    pub plan: SkillPackageDeletePlan,
    pub deleted: bool,
    pub remaining_skill: Option<SkillDescriptor>,
    pub sandbox_cache: Option<SkillSandboxCache>,
}

fn merge_activation_policies(
    runtime_policy: SkillActivationPolicy,
    persona_policy: &SkillActivationPolicy,
) -> SkillActivationPolicy {
    runtime_policy.and(persona_policy)
}

#[cfg(test)]
mod tests {
    use crate::{
        SkillActivationConfig, SkillActivationPolicy, SkillCatalog, SkillDescriptor,
        SkillPackageError, SkillRuntimeInstallRequest, SkillRuntimeSnapshot, SkillSandboxCache,
        SkillSandboxEntry,
    };

    #[test]
    fn runtime_prompt_inventory_merges_persona_allowlist_activation_and_sandbox_cache() {
        let catalog = SkillCatalog::from_skills([
            SkillDescriptor::new("writer", "C:/skills/writer/SKILL.md")
                .with_description("Local writer"),
            SkillDescriptor::new("draw", "C:/skills/draw/SKILL.md").with_description("Draw"),
        ]);
        let mut activation = SkillActivationConfig::new();
        activation
            .set_active(&catalog, "draw", false)
            .expect("draw should disable");
        let snapshot = SkillRuntimeSnapshot::new(catalog)
            .with_activation_config(activation)
            .with_sandbox_cache(
                SkillSandboxCache::from_entries([
                    SkillSandboxEntry::new("sandbox").with_description("Sandbox preset")
                ]),
                true,
            );

        let inventory = snapshot.prompt_inventory(
            &SkillActivationPolicy::all_enabled().allow_only(["writer", "draw", "sandbox"]),
        );

        let names = inventory
            .skills()
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["sandbox", "writer"]);
    }

    #[test]
    fn runtime_install_delete_syncs_catalog_activation_and_sandbox_cache() {
        let mut snapshot = SkillRuntimeSnapshot::new(SkillCatalog::new());
        let installed = snapshot
            .install_package(
                SkillRuntimeInstallRequest::new(["writer/SKILL.md"])
                    .with_description("Writer")
                    .with_manifest_path("data/skills/writer/SKILL.md"),
            )
            .expect("install should update runtime snapshot");

        assert_eq!(installed.plan.skill_name, "writer");
        assert_eq!(installed.skill.path, "data/skills/writer/SKILL.md");
        assert!(snapshot.catalog.skill("writer").is_some());
        assert!(
            snapshot
                .sandbox_cache
                .as_ref()
                .expect("sandbox cache")
                .entry("writer")
                .is_some()
        );

        snapshot
            .set_active("writer", false)
            .expect("activation should update");
        assert!(!snapshot.activation.is_active("writer"));

        let deleted = snapshot
            .delete_package("writer")
            .expect("delete should update runtime snapshot");
        assert!(deleted.deleted);
        assert!(snapshot.catalog.skill("writer").is_none());
        assert!(snapshot.activation.is_active("writer"));
        assert!(
            snapshot
                .sandbox_cache
                .as_ref()
                .expect("sandbox cache")
                .entry("writer")
                .is_none()
        );
    }

    #[test]
    fn runtime_install_rejects_existing_without_overwrite() {
        let catalog = SkillCatalog::from_skills([SkillDescriptor::new(
            "writer",
            "C:/skills/writer/SKILL.md",
        )]);
        let mut snapshot = SkillRuntimeSnapshot::new(catalog);

        let error = snapshot
            .install_package(
                SkillRuntimeInstallRequest::new(["writer/SKILL.md"]).with_overwrite(false),
            )
            .expect_err("existing skill should require overwrite");

        assert!(matches!(
            error,
            SkillPackageError::SkillAlreadyExists { .. }
        ));
    }
}
