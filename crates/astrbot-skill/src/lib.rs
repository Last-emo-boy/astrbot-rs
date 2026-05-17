mod activation;
mod catalog;
mod frontmatter;
mod package;
mod prompt;
mod prompt_inventory;
mod sandbox_cache;

pub use activation::{
    SkillActivationChange, SkillActivationConfig, SkillActivationPolicy, SkillActivationState,
    SkillPackageOperation,
};
pub use catalog::{SkillCatalog, SkillDescriptor, SkillSource, is_valid_skill_name};
pub use frontmatter::parse_frontmatter_description;
pub use package::{
    SkillPackageDeletePlan, SkillPackageError, SkillPackageInstallPlan, SkillZipValidation,
    ensure_local_package_mutation, validate_skill_zip_entries,
};
pub use prompt::{
    SkillPromptInventory, SkillPromptRenderer, SkillPromptRuntime,
    build_skill_read_command_example, sanitize_prompt_description, sanitize_prompt_path_for_prompt,
    sanitize_skill_display_name,
};
pub use sandbox_cache::{
    SANDBOX_SKILLS_CACHE_VERSION, SANDBOX_SKILLS_ROOT, SANDBOX_WORKSPACE_ROOT, SkillSandboxCache,
    SkillSandboxCacheStatus, SkillSandboxEntry, default_sandbox_skill_path,
};
