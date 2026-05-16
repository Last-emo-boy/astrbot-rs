use crate::{SkillDescriptor, SkillSource};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SkillPromptRuntime {
    #[default]
    Local,
    Sandbox,
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
            sandbox_workspace: "/workspace".to_string(),
            sandbox_skills_root: "skills".to_string(),
        }
    }

    pub fn with_runtime(mut self, runtime: SkillPromptRuntime) -> Self {
        self.runtime = runtime;
        self
    }

    pub fn render(&self, skills: &[SkillDescriptor]) -> Option<String> {
        if skills.is_empty() {
            return None;
        }

        let mut skills = skills.to_vec();
        skills.sort_by(|left, right| left.name.cmp(&right.name));
        let lines = skills
            .iter()
            .map(|skill| {
                format!(
                    "- **{}**: {}\n  File: `{}`",
                    sanitize_skill_name(&skill.name),
                    sanitize_description(&skill.description),
                    self.render_path(skill)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        Some(format!(
            "## Skills\n\nYou have specialized skills available for this session. Read the referenced `SKILL.md` before using a skill.\n\n### Available skills\n\n{lines}\n\n### Skill rules\n\n1. Use a skill when the user names it or the task clearly matches its description.\n2. Load only the referenced `SKILL.md` and directly linked files.\n3. If a skill cannot be applied, state the issue and continue with the best alternative.\n"
        ))
    }

    fn render_path(&self, skill: &SkillDescriptor) -> String {
        match (self.runtime, skill.source) {
            (SkillPromptRuntime::Sandbox, SkillSource::Sandbox | SkillSource::Synced) => format!(
                "{}/{}/{}/SKILL.md",
                self.sandbox_workspace,
                self.sandbox_skills_root,
                sanitize_skill_name(&skill.name)
            ),
            _ => sanitize_path(&skill.path),
        }
    }
}

fn sanitize_skill_name(name: &str) -> String {
    if !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        name.to_string()
    } else {
        "<invalid_skill_name>".to_string()
    }
}

fn sanitize_description(description: &str) -> String {
    let sanitized = description
        .replace('`', "")
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>();
    let sanitized = sanitized.split_whitespace().collect::<Vec<_>>().join(" ");
    if sanitized.is_empty() {
        "Read SKILL.md for details.".to_string()
    } else {
        sanitized
    }
}

fn sanitize_path(path: &str) -> String {
    path.replace('`', "")
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::{SkillPromptRenderer, SkillPromptRuntime};
    use crate::{SkillDescriptor, SkillSource};

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
}
