use std::collections::BTreeSet;

use astrbot_core::{MessageEvent, ProviderRequest, Result};
use astrbot_skill::{
    SkillActivationPolicy, SkillCatalog, SkillDescriptor, SkillPromptInventory,
    SkillPromptRenderer, SkillRuntimeSnapshot,
};
use async_trait::async_trait;

use crate::{AgentPersona, ProviderRequestDecorator};

pub struct SkillPromptInventoryRequestDecorator {
    inventory: SkillPromptInventory,
    renderer: SkillPromptRenderer,
    allowed_skills: Option<Vec<String>>,
}

impl SkillPromptInventoryRequestDecorator {
    pub fn new(inventory: SkillPromptInventory) -> Self {
        Self {
            inventory,
            renderer: SkillPromptRenderer::new(),
            allowed_skills: None,
        }
    }

    pub fn for_persona(inventory: SkillPromptInventory, persona: &AgentPersona) -> Self {
        Self::new(inventory).with_allowed_skills(persona.skills.clone())
    }

    pub fn from_catalog_for_persona(
        catalog: &SkillCatalog,
        activation_policy: &SkillActivationPolicy,
        persona: &AgentPersona,
    ) -> Self {
        Self::new(SkillPromptInventory::from_catalog(
            catalog,
            &activation_policy
                .clone()
                .and(&persona.skill_activation_policy()),
        ))
    }

    pub fn from_runtime_for_persona(
        runtime: &SkillRuntimeSnapshot,
        persona: &AgentPersona,
    ) -> Self {
        Self::new(runtime.prompt_inventory(&persona.skill_activation_policy()))
    }

    pub fn with_renderer(mut self, renderer: SkillPromptRenderer) -> Self {
        self.renderer = renderer;
        self
    }

    pub fn with_allowed_skills(mut self, allowed_skills: Option<Vec<String>>) -> Self {
        self.allowed_skills = allowed_skills;
        self
    }
}

#[async_trait]
impl ProviderRequestDecorator for SkillPromptInventoryRequestDecorator {
    async fn decorate(&self, _event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        let skills = filter_allowed_skills(self.inventory.skills(), self.allowed_skills.as_deref());
        let Some(skills_prompt) = self.renderer.render(&skills) else {
            return Ok(());
        };

        request.system_prompt = match request.system_prompt.take() {
            Some(prompt) if !prompt.trim().is_empty() => {
                Some(format!("{}\n\n{}", prompt.trim_end(), skills_prompt))
            }
            _ => Some(skills_prompt),
        };
        Ok(())
    }
}

fn filter_allowed_skills(
    skills: &[SkillDescriptor],
    allowed_skills: Option<&[String]>,
) -> Vec<SkillDescriptor> {
    let Some(allowed_skills) = allowed_skills else {
        return skills.to_vec();
    };
    let allowed = allowed_skills.iter().collect::<BTreeSet<_>>();
    skills
        .iter()
        .filter(|skill| allowed.contains(&skill.name))
        .cloned()
        .collect()
}
