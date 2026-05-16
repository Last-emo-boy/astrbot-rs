use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::{ResolvedSubagent, SubagentConfigSource, SubagentPersonaProfile};

#[derive(Clone, Debug, PartialEq)]
pub struct HandoffToolSpec {
    pub name: String,
    pub agent_name: String,
    pub description: String,
    pub parameters: Value,
    pub provider_id: Option<String>,
    pub persona_id: Option<String>,
    pub tools: Option<Vec<String>>,
    pub instructions: String,
    pub begin_dialogs: Vec<String>,
}

impl HandoffToolSpec {
    pub fn from_subagent(subagent: ResolvedSubagent) -> Self {
        let name = format!("transfer_to_{}", sanitize_tool_fragment(&subagent.name));
        let description = subagent.public_description.clone().unwrap_or_else(|| {
            format!(
                "Delegate tasks to {} agent to handle the request.",
                subagent.name
            )
        });

        Self {
            name,
            agent_name: subagent.name,
            description,
            parameters: default_handoff_parameters(),
            provider_id: subagent.provider_id,
            persona_id: subagent.persona_id,
            tools: subagent.tools,
            instructions: subagent.instructions,
            begin_dialogs: subagent.begin_dialogs,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HandoffRegistration {
    handoffs: Vec<HandoffToolSpec>,
}

impl HandoffRegistration {
    pub fn new(handoffs: Vec<HandoffToolSpec>) -> Self {
        Self { handoffs }
    }

    pub fn handoffs(&self) -> &[HandoffToolSpec] {
        &self.handoffs
    }

    pub fn into_handoffs(self) -> Vec<HandoffToolSpec> {
        self.handoffs
    }
}

#[async_trait]
pub trait SubagentResolver: Send + Sync {
    async fn persona(&self, persona_id: &str) -> Result<Option<SubagentPersonaProfile>>;
}

#[derive(Default)]
pub struct InMemoryHandoffRegistry {
    handoffs: RwLock<Vec<HandoffToolSpec>>,
}

impl InMemoryHandoffRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn replace(&self, handoffs: Vec<HandoffToolSpec>) -> Result<()> {
        *self.handoffs.write().map_err(lock_error)? = handoffs;
        Ok(())
    }

    pub fn handoffs(&self) -> Vec<HandoffToolSpec> {
        self.handoffs.read().expect("handoffs lock").clone()
    }
}

#[derive(Default)]
pub struct StaticSubagentResolver {
    personas: RwLock<HashMap<String, SubagentPersonaProfile>>,
}

impl StaticSubagentResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_persona(&self, persona: SubagentPersonaProfile) -> Result<()> {
        self.personas
            .write()
            .map_err(lock_error)?
            .insert(persona.persona_id.clone(), persona);
        Ok(())
    }
}

#[async_trait]
impl SubagentResolver for StaticSubagentResolver {
    async fn persona(&self, persona_id: &str) -> Result<Option<SubagentPersonaProfile>> {
        Ok(self
            .personas
            .read()
            .map_err(lock_error)?
            .get(persona_id)
            .cloned())
    }
}

pub struct SubagentOrchestrator {
    resolver: Arc<dyn SubagentResolver>,
    registry: Arc<InMemoryHandoffRegistry>,
}

impl SubagentOrchestrator {
    pub fn new(
        resolver: Arc<dyn SubagentResolver>,
        registry: Arc<InMemoryHandoffRegistry>,
    ) -> Self {
        Self { resolver, registry }
    }

    pub async fn reload_from_config(
        &self,
        source: &SubagentConfigSource,
    ) -> Result<HandoffRegistration> {
        let mut handoffs = Vec::new();
        for config in source.enabled_agents() {
            let persona = match config.persona_id.as_deref() {
                Some(persona_id) => self.resolver.persona(persona_id).await?,
                None => None,
            };
            let subagent = ResolvedSubagent::from_config(config, persona);
            handoffs.push(HandoffToolSpec::from_subagent(subagent));
        }

        handoffs.sort_by(|left, right| left.name.cmp(&right.name));
        self.registry.replace(handoffs.clone())?;
        Ok(HandoffRegistration::new(handoffs))
    }

    pub fn registry(&self) -> Arc<InMemoryHandoffRegistry> {
        self.registry.clone()
    }
}

fn default_handoff_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "input": {
                "type": "string",
                "description": "The input to hand off to another agent."
            },
            "image_urls": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Optional image references for multimodal subagent tasks."
            },
            "background_task": {
                "type": "boolean",
                "description": "Set true when the subagent task can run in the background."
            }
        },
        "required": ["input"]
    })
}

fn sanitize_tool_fragment(name: &str) -> String {
    let fragment = name
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let fragment = fragment.trim_matches('_').to_string();
    if fragment.is_empty() {
        "subagent".to_string()
    } else {
        fragment
    }
}

fn lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("subagent orchestrator lock: {err}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::subagent::orchestrator::StaticSubagentResolver;
    use crate::{
        InMemoryHandoffRegistry, SubagentConfig, SubagentConfigSource, SubagentOrchestrator,
        SubagentPersonaProfile,
    };

    #[tokio::test]
    async fn orchestrator_registers_handoff_specs_without_executing_agents() {
        let resolver = Arc::new(StaticSubagentResolver::new());
        resolver
            .add_persona(
                SubagentPersonaProfile::new("persona-a")
                    .with_system_prompt("persona prompt")
                    .with_tools(["search"]),
            )
            .expect("persona should save");
        let registry = Arc::new(InMemoryHandoffRegistry::new());
        let orchestrator = SubagentOrchestrator::new(resolver, registry.clone());

        let registration = orchestrator
            .reload_from_config(&SubagentConfigSource::new(vec![
                SubagentConfig::new("data analyst")
                    .with_persona_id("persona-a")
                    .with_provider_id("openai-fast"),
            ]))
            .await
            .expect("handoffs should register");

        assert_eq!(registration.handoffs().len(), 1);
        assert_eq!(registration.handoffs()[0].name, "transfer_to_data_analyst");
        assert_eq!(
            registration.handoffs()[0].provider_id.as_deref(),
            Some("openai-fast")
        );
        assert_eq!(registration.handoffs()[0].instructions, "persona prompt");
        assert_eq!(registry.handoffs(), registration.handoffs());
    }
}
