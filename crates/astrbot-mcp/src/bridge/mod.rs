use astrbot_tool::{ToolCatalog, ToolDescriptor, ToolSource};
use serde_json::json;

use crate::{
    McpJsonSchema, McpPrompt, McpResource, McpResourceTemplate, McpServerName, McpTool,
    build_mcp_prompt_tool_names, build_mcp_resource_tool_names,
};

#[derive(Clone, Debug, PartialEq)]
pub struct McpBridgeRegistration {
    pub server_name: McpServerName,
    pub descriptors: Vec<ToolDescriptor>,
}

impl McpBridgeRegistration {
    pub fn into_catalog(self) -> ToolCatalog {
        let mut catalog = ToolCatalog::new();
        for descriptor in self.descriptors {
            catalog.add_tool(descriptor);
        }
        catalog
    }
}

#[derive(Clone, Debug)]
pub struct McpBridgeCatalogBuilder {
    server_name: McpServerName,
}

impl McpBridgeCatalogBuilder {
    pub fn new(server_name: McpServerName) -> Self {
        Self { server_name }
    }

    pub fn build_registration(
        &self,
        tools: &[McpTool],
        resources: &[McpResource],
        resource_templates: &[McpResourceTemplate],
        prompts: &[McpPrompt],
    ) -> McpBridgeRegistration {
        let mut descriptors = tools
            .iter()
            .map(|tool| self.tool_descriptor(tool))
            .collect::<Vec<_>>();
        if !resources.is_empty() || !resource_templates.is_empty() {
            descriptors.extend(self.resource_descriptors(!resource_templates.is_empty()));
        }
        descriptors.extend(self.prompt_descriptors(!prompts.is_empty()));

        McpBridgeRegistration {
            server_name: self.server_name.clone(),
            descriptors,
        }
    }

    fn tool_descriptor(&self, tool: &McpTool) -> ToolDescriptor {
        ToolDescriptor::new(scoped_name(self.server_name.as_str(), &tool.name))
            .with_description(
                tool.description
                    .clone()
                    .or_else(|| tool.title.clone())
                    .unwrap_or_else(|| format!("MCP tool {}", tool.name)),
            )
            .with_parameters(tool.input_schema.as_json().clone())
            .with_source(ToolSource::Mcp)
    }

    fn resource_descriptors(&self, include_templates: bool) -> Vec<ToolDescriptor> {
        build_mcp_resource_tool_names(self.server_name.as_str(), include_templates)
            .into_iter()
            .map(|name| {
                let parameters = if name.ends_with("read_resource") {
                    json!({
                        "type": "object",
                        "properties": {
                            "uri": {"type": "string"}
                        },
                        "required": ["uri"]
                    })
                } else {
                    McpJsonSchema::object().as_json().clone()
                };
                ToolDescriptor::new(name)
                    .with_description("Synthetic MCP resource bridge tool")
                    .with_parameters(parameters)
                    .with_source(ToolSource::Mcp)
            })
            .collect()
    }

    fn prompt_descriptors(&self, include_prompts: bool) -> Vec<ToolDescriptor> {
        if !include_prompts {
            return Vec::new();
        }
        build_mcp_prompt_tool_names(self.server_name.as_str())
            .into_iter()
            .map(|name| {
                ToolDescriptor::new(name)
                    .with_description("Synthetic MCP prompt bridge tool")
                    .with_parameters(McpJsonSchema::object().as_json().clone())
                    .with_source(ToolSource::Mcp)
            })
            .collect()
    }
}

fn scoped_name(server_name: &str, tool_name: &str) -> String {
    let prefix = build_mcp_resource_tool_names(server_name, false)[0]
        .trim_end_matches("_list_resources")
        .to_string();
    let tool_name = tool_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    format!("{prefix}_{tool_name}")
}

#[cfg(test)]
mod tests {
    use astrbot_tool::ToolSource;
    use serde_json::json;

    use super::McpBridgeCatalogBuilder;
    use crate::{
        McpJsonSchema, McpPrompt, McpResource, McpResourceTemplate, McpServerName, McpTool, McpUri,
    };

    #[test]
    fn bridge_registers_mcp_tools_resources_and_prompts_as_catalog_descriptors() {
        let builder =
            McpBridgeCatalogBuilder::new(McpServerName::new("Docs Server").expect("server name"));
        let tool = McpTool::new("Search Docs")
            .with_description("Search documentation")
            .with_input_schema(McpJsonSchema::from_json(json!({
                "type": "object",
                "properties": {"query": {"type": "string"}}
            })));
        let resource = McpResource::new(
            McpUri::new("file:///docs/readme.md").expect("uri"),
            "readme",
        );
        let template = McpResourceTemplate {
            uri_template: "file:///docs/{name}".to_string(),
            name: "doc".to_string(),
            title: None,
            description: None,
            mime_type: None,
        };
        let prompt = McpPrompt::new("summarize");

        let catalog = builder
            .build_registration(&[tool], &[resource], &[template], &[prompt])
            .into_catalog();
        let names = catalog
            .tools()
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"mcp_docs_server_search_docs"));
        assert!(names.contains(&"mcp_docs_server_read_resource"));
        assert!(names.contains(&"mcp_docs_server_get_prompt"));
        assert!(
            catalog
                .tools()
                .iter()
                .all(|tool| tool.source == ToolSource::Mcp)
        );
    }
}
