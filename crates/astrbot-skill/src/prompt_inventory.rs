use crate::{
    SANDBOX_SKILLS_ROOT, SANDBOX_WORKSPACE_ROOT, SkillActivationPolicy, SkillCatalog,
    SkillDescriptor, SkillSource, default_sandbox_skill_path,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SkillPromptRuntime {
    #[default]
    Local,
    Sandbox,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillPromptInventory {
    skills: Vec<SkillDescriptor>,
}

impl SkillPromptInventory {
    pub fn new(skills: impl IntoIterator<Item = SkillDescriptor>) -> Self {
        let mut skills = skills.into_iter().collect::<Vec<_>>();
        skills.sort_by(|left, right| left.name.cmp(&right.name));
        Self { skills }
    }

    pub fn from_catalog(catalog: &SkillCatalog, policy: &SkillActivationPolicy) -> Self {
        Self::new(catalog.active_skills(policy))
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn skills(&self) -> &[SkillDescriptor] {
        &self.skills
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillPromptRenderer {
    runtime: SkillPromptRuntime,
    sandbox_workspace: String,
    sandbox_skills_root: String,
}

impl Default for SkillPromptRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillPromptRenderer {
    pub fn new() -> Self {
        Self {
            runtime: SkillPromptRuntime::Local,
            sandbox_workspace: SANDBOX_WORKSPACE_ROOT.to_string(),
            sandbox_skills_root: SANDBOX_SKILLS_ROOT.to_string(),
        }
    }

    pub fn with_runtime(mut self, runtime: SkillPromptRuntime) -> Self {
        self.runtime = runtime;
        self
    }

    pub fn render_inventory(&self, inventory: &SkillPromptInventory) -> Option<String> {
        self.render(inventory.skills())
    }

    pub fn render(&self, skills: &[SkillDescriptor]) -> Option<String> {
        if skills.is_empty() {
            return None;
        }

        let inventory = SkillPromptInventory::new(skills.to_vec());
        let mut example_path = String::new();
        let lines = inventory
            .skills()
            .iter()
            .map(|skill| {
                let rendered_path = self.render_path(skill);
                if example_path.is_empty() {
                    example_path = rendered_path.clone();
                }
                let description = sanitize_prompt_description(&skill.description);
                let description = if description.is_empty() {
                    "Read SKILL.md for details.".to_string()
                } else {
                    description
                };
                format!(
                    "- **{}**: {}\n  File: `{}`",
                    sanitize_skill_display_name(&skill.name),
                    description,
                    rendered_path
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let example_command =
            build_skill_read_command_example(&sanitize_prompt_path_for_prompt(&example_path));

        Some(format!(
            "## Skills\n\nYou have specialized skills - reusable instruction bundles stored in `SKILL.md` files. Each skill has a name and description that tells you what it does and when to use it.\n\n### Available skills\n\n{lines}\n\n### Skill rules\n\n1. Discovery - The list above is the complete skill inventory for this session. Full instructions are in the referenced `SKILL.md` file.\n2. When to trigger - Use a skill if the user names it explicitly, or if the task clearly matches the skill's description.\n3. Mandatory grounding - Before executing any skill, first read its `SKILL.md` using the absolute path shown above, for example `{example_command}`.\n4. Progressive disclosure - Load only what is directly referenced from `SKILL.md`; do not bulk-load every file in the skill directory.\n5. Failure handling - If a skill cannot be applied, state the issue and continue with the best alternative.\n"
        ))
    }

    fn render_path(&self, skill: &SkillDescriptor) -> String {
        match (self.runtime, skill.source) {
            (SkillPromptRuntime::Sandbox, SkillSource::Sandbox | SkillSource::Synced) => {
                sanitize_prompt_path_for_prompt(&format!(
                    "{}/{}/{}/SKILL.md",
                    self.sandbox_workspace,
                    self.sandbox_skills_root,
                    sanitize_skill_display_name(&skill.name)
                ))
            }
            _ if skill.source == SkillSource::Sandbox => {
                sanitize_prompt_path_for_prompt(&default_sandbox_skill_path(&skill.name))
            }
            _ => sanitize_prompt_path_for_prompt(&skill.path),
        }
    }
}

pub fn sanitize_skill_display_name(name: &str) -> String {
    if crate::is_valid_skill_name(name) {
        name.to_string()
    } else {
        "<invalid_skill_name>".to_string()
    }
}

pub fn sanitize_prompt_description(description: &str) -> String {
    let description = description
        .replace('`', "")
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>();
    description.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn sanitize_prompt_path_for_prompt(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }

    let path = path.replace('\\', "/").replace('`', "");
    let mut sanitized = String::with_capacity(path.len());
    for ch in path.chars().filter(|ch| !ch.is_control()) {
        if ch.is_alphanumeric()
            || matches!(
                ch,
                '_' | '.' | '/' | ' ' | ',' | '(' | ')' | '\'' | '-' | ':'
            )
        {
            sanitized.push(ch);
        }
    }
    sanitized
}

pub fn build_skill_read_command_example(path: &str) -> String {
    if path.is_empty() || path == "<skills_root>/<skill_name>/SKILL.md" {
        return "cat <skills_root>/<skill_name>/SKILL.md".to_string();
    }

    if is_windows_prompt_path(path) {
        format!("type \"{path}\"")
    } else {
        format!("cat {}", shell_quote(path))
    }
}

fn is_windows_prompt_path(path: &str) -> bool {
    has_windows_drive_prefix(path) || path.starts_with("//")
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn shell_quote(path: &str) -> String {
    if path
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '/' | '-'))
    {
        path.to_string()
    } else {
        format!("'{}'", path.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SkillPromptInventory, SkillPromptRenderer, SkillPromptRuntime,
        sanitize_prompt_path_for_prompt,
    };
    use crate::{SkillActivationPolicy, SkillCatalog, SkillDescriptor, SkillSource};

    #[test]
    fn skill_prompt_renderer_lists_sanitized_skill_metadata() {
        let prompt = SkillPromptRenderer::new()
            .render(&[
                SkillDescriptor::new("writer", "C:\\skills\\writer\\SKILL.md")
                    .with_description("Draft `clean` text"),
                SkillDescriptor::new("bad name", "/skills/bad/SKILL.md"),
            ])
            .expect("prompt should render");

        assert!(prompt.contains("**writer**"));
        assert!(prompt.contains("Draft clean text"));
        assert!(prompt.contains("C:/skills/writer/SKILL.md"));
        assert!(prompt.contains("<invalid_skill_name>"));
    }

    #[test]
    fn skill_prompt_renderer_uses_sandbox_paths_for_sandbox_skills() {
        let prompt = SkillPromptRenderer::new()
            .with_runtime(SkillPromptRuntime::Sandbox)
            .render(&[SkillDescriptor::new("preset", "ignored").with_source(SkillSource::Sandbox)])
            .expect("prompt should render");

        assert!(prompt.contains("/workspace/skills/preset/SKILL.md"));
    }

    #[test]
    fn skill_prompt_inventory_consumes_active_catalog_entries() {
        let mut catalog = SkillCatalog::new();
        catalog.add_skill(SkillDescriptor::new("writer", "/skills/writer/SKILL.md"));
        catalog.add_skill(SkillDescriptor::new("draw", "/skills/draw/SKILL.md"));
        let inventory = SkillPromptInventory::from_catalog(
            &catalog,
            &SkillActivationPolicy::all_enabled().disable("draw"),
        );

        assert_eq!(inventory.skills().len(), 1);
        assert_eq!(inventory.skills()[0].name, "writer");
    }

    #[test]
    fn prompt_path_sanitization_removes_control_backticks_and_injection_chars() {
        let sanitized =
            sanitize_prompt_path_for_prompt("C:\\skills\\`bad`\\SKILL.md\u{7}; rm -rf /");

        assert_eq!(sanitized, "C:/skills/bad/SKILL.md rm -rf /");
    }
}
