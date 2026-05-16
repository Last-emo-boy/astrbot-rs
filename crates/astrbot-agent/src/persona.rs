use astrbot_core::{MessageEvent, ProviderRequest, Result};
use async_trait::async_trait;

use crate::ProviderRequestDecorator;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentPersona {
    pub id: String,
    pub system_prompt: Option<String>,
}

impl AgentPersona {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            system_prompt: None,
        }
    }

    pub fn with_system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        let system_prompt = system_prompt.into();
        self.system_prompt = (!system_prompt.trim().is_empty()).then_some(system_prompt);
        self
    }
}

pub struct PersonaPromptDecorator {
    persona: AgentPersona,
}

impl PersonaPromptDecorator {
    pub fn new(persona: AgentPersona) -> Self {
        Self { persona }
    }
}

#[async_trait]
impl ProviderRequestDecorator for PersonaPromptDecorator {
    async fn decorate(&self, _event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        if request
            .system_prompt
            .as_deref()
            .is_some_and(|prompt| !prompt.trim().is_empty())
        {
            return Ok(());
        }

        request.system_prompt = self.persona.system_prompt.clone();
        Ok(())
    }
}
