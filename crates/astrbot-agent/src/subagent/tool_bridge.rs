use astrbot_tool::{ToolCatalog, ToolDescriptor, ToolSource};

use crate::{HandoffRegistration, HandoffToolSpec};

#[derive(Clone, Debug, Default)]
pub struct HandoffToolBridge;

impl HandoffToolBridge {
    pub fn descriptor(spec: &HandoffToolSpec) -> ToolDescriptor {
        ToolDescriptor::new(spec.name.clone())
            .with_description(spec.description.clone())
            .with_parameters(spec.parameters.clone())
            .with_source(ToolSource::Handoff)
    }

    pub fn descriptors(registration: &HandoffRegistration) -> Vec<ToolDescriptor> {
        registration
            .handoffs()
            .iter()
            .map(Self::descriptor)
            .collect()
    }

    pub fn extend_catalog(catalog: &mut ToolCatalog, registration: &HandoffRegistration) {
        for descriptor in Self::descriptors(registration) {
            catalog.add_tool(descriptor);
        }
    }
}

#[cfg(test)]
mod tests {
    use astrbot_tool::{ToolActivationPolicy, ToolCatalog, ToolSource};

    use crate::{HandoffRegistration, HandoffToolBridge, HandoffToolSpec};

    #[test]
    fn handoff_bridge_adds_handoff_descriptors_without_generic_execution_state() {
        let registration = HandoffRegistration::new(vec![HandoffToolSpec {
            name: "transfer_to_writer".to_string(),
            agent_name: "writer".to_string(),
            description: "draft copy".to_string(),
            parameters: serde_json::json!({"type": "object"}),
            provider_id: Some("openai-fast".to_string()),
            persona_id: Some("persona-writer".to_string()),
            tools: Some(vec!["search".to_string()]),
            instructions: "write clearly".to_string(),
            begin_dialogs: Vec::new(),
        }]);
        let mut catalog = ToolCatalog::new();

        HandoffToolBridge::extend_catalog(&mut catalog, &registration);

        let tools = catalog.active_tools(&ToolActivationPolicy::new());
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "transfer_to_writer");
        assert_eq!(tools[0].source, ToolSource::Handoff);
        assert_eq!(tools[0].description.as_deref(), Some("draft copy"));
        assert_eq!(tools[0].parameters, serde_json::json!({"type": "object"}));
    }
}
