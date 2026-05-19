use std::collections::BTreeMap;

use astrbot_tool::{ToolCatalog, ToolDescriptor, ToolSourceMetadata};
use serde_json::json;

use crate::{
    McpClientBoundary, McpCursor, McpError, McpGetPromptRequest, McpJsonObject, McpJsonSchema,
    McpJsonValue, McpPrompt, McpReadResourceRequest, McpResource, McpResourceTemplate, McpResult,
    McpServerName, McpTool, McpToolArguments, McpToolCallRequest, McpToolCallResult, McpUri,
    build_mcp_prompt_tool_names, build_mcp_resource_tool_names,
};

#[derive(Clone, Debug, PartialEq)]
pub struct McpBridgeRegistration {
    pub server_name: McpServerName,
    pub descriptors: Vec<ToolDescriptor>,
    routes: BTreeMap<String, McpBridgeRoute>,
}

impl McpBridgeRegistration {
    pub fn into_catalog(self) -> ToolCatalog {
        let mut catalog = ToolCatalog::new();
        for descriptor in self.descriptors {
            catalog.add_tool(descriptor);
        }
        catalog
    }

    pub fn resolve_call(
        &self,
        descriptor_name: &str,
        arguments: McpJsonObject,
    ) -> McpResult<McpBridgeCall> {
        let route = self.routes.get(descriptor_name).ok_or_else(|| {
            McpError::Unsupported(format!(
                "MCP bridge descriptor '{}' is not registered for server '{}'",
                descriptor_name,
                self.server_name.as_str()
            ))
        })?;
        route.resolve(arguments)
    }

    pub async fn execute_call<C>(
        &self,
        client: &C,
        descriptor_name: &str,
        arguments: McpJsonObject,
    ) -> McpResult<McpToolCallResult>
    where
        C: McpClientBoundary + ?Sized,
    {
        match self.resolve_call(descriptor_name, arguments)? {
            McpBridgeCall::Tool(request) => client.call_tool(request).await,
            McpBridgeCall::ListResources { cursor } => {
                let page = client.list_resources(cursor).await?;
                Ok(format_resources_listing(self.server_name.as_str(), &page).into())
            }
            McpBridgeCall::ReadResource(request) => {
                let result = client.read_resource(request.clone()).await?;
                Ok(result.into_tool_result(self.server_name.as_str(), &request.uri))
            }
            McpBridgeCall::ListResourceTemplates { cursor } => {
                let page = client.list_resource_templates(cursor).await?;
                Ok(format_resource_templates_listing(self.server_name.as_str(), &page).into())
            }
            McpBridgeCall::ListPrompts { cursor } => {
                let page = client.list_prompts(cursor).await?;
                Ok(format_prompts_listing(self.server_name.as_str(), &page).into())
            }
            McpBridgeCall::GetPrompt(request) => {
                let result = client.get_prompt(request.clone()).await?;
                Ok(crate::prompts::shape_get_prompt_result(
                    self.server_name.as_str(),
                    &request.name,
                    &result,
                )
                .into())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum McpBridgeCall {
    Tool(McpToolCallRequest),
    ListResources { cursor: Option<McpCursor> },
    ReadResource(McpReadResourceRequest),
    ListResourceTemplates { cursor: Option<McpCursor> },
    ListPrompts { cursor: Option<McpCursor> },
    GetPrompt(McpGetPromptRequest),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum McpBridgeRoute {
    Tool { original_name: String },
    ListResources,
    ReadResource,
    ListResourceTemplates,
    ListPrompts,
    GetPrompt,
}

impl McpBridgeRoute {
    fn resolve(&self, arguments: McpJsonObject) -> McpResult<McpBridgeCall> {
        match self {
            Self::Tool { original_name } => Ok(McpBridgeCall::Tool(McpToolCallRequest {
                name: original_name.clone(),
                arguments: McpToolArguments(arguments),
                timeout_seconds: None,
            })),
            Self::ListResources => Ok(McpBridgeCall::ListResources {
                cursor: optional_cursor(&arguments),
            }),
            Self::ReadResource => {
                let uri = required_string_argument(&arguments, "uri")?;
                Ok(McpBridgeCall::ReadResource(McpReadResourceRequest::new(
                    McpUri::new(uri)?,
                )))
            }
            Self::ListResourceTemplates => Ok(McpBridgeCall::ListResourceTemplates {
                cursor: optional_cursor(&arguments),
            }),
            Self::ListPrompts => Ok(McpBridgeCall::ListPrompts {
                cursor: optional_cursor(&arguments),
            }),
            Self::GetPrompt => {
                let name = required_string_argument(&arguments, "name")?;
                let mut request = McpGetPromptRequest::new(name);
                if let Some(McpJsonValue::Object(prompt_args)) = arguments.get("arguments") {
                    request.arguments = prompt_args.clone();
                }
                Ok(McpBridgeCall::GetPrompt(request))
            }
        }
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
        self.build_registration_with_capabilities(
            tools,
            resources,
            resource_templates,
            prompts,
            !resources.is_empty() || !resource_templates.is_empty(),
            !prompts.is_empty(),
        )
    }

    pub fn build_registration_with_capabilities(
        &self,
        tools: &[McpTool],
        _resources: &[McpResource],
        resource_templates: &[McpResourceTemplate],
        _prompts: &[McpPrompt],
        supports_resources: bool,
        supports_prompts: bool,
    ) -> McpBridgeRegistration {
        let mut routes = BTreeMap::new();
        let mut descriptors = tools
            .iter()
            .map(|tool| {
                let descriptor = self.tool_descriptor(tool);
                routes.insert(
                    descriptor.name.clone(),
                    McpBridgeRoute::Tool {
                        original_name: tool.name.clone(),
                    },
                );
                descriptor
            })
            .collect::<Vec<_>>();
        if supports_resources {
            let resource_descriptors = self.resource_descriptors(!resource_templates.is_empty());
            for descriptor in &resource_descriptors {
                let route = if descriptor.name.ends_with("_read_resource") {
                    McpBridgeRoute::ReadResource
                } else if descriptor.name.ends_with("_list_resource_templates") {
                    McpBridgeRoute::ListResourceTemplates
                } else {
                    McpBridgeRoute::ListResources
                };
                routes.insert(descriptor.name.clone(), route);
            }
            descriptors.extend(resource_descriptors);
        }
        let prompt_descriptors = self.prompt_descriptors(supports_prompts);
        for descriptor in &prompt_descriptors {
            let route = if descriptor.name.ends_with("_get_prompt") {
                McpBridgeRoute::GetPrompt
            } else {
                McpBridgeRoute::ListPrompts
            };
            routes.insert(descriptor.name.clone(), route);
        }
        descriptors.extend(prompt_descriptors);

        McpBridgeRegistration {
            server_name: self.server_name.clone(),
            descriptors,
            routes,
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
            .with_source_metadata(ToolSourceMetadata::mcp(self.server_name.as_str()))
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
                    .with_source_metadata(ToolSourceMetadata::mcp(self.server_name.as_str()))
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
                    .with_source_metadata(ToolSourceMetadata::mcp(self.server_name.as_str()))
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

fn optional_cursor(arguments: &McpJsonObject) -> Option<McpCursor> {
    arguments
        .get("cursor")
        .and_then(McpJsonValue::as_str)
        .and_then(McpCursor::new)
}

fn required_string_argument(arguments: &McpJsonObject, key: &str) -> McpResult<String> {
    arguments
        .get(key)
        .and_then(McpJsonValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| McpError::InvalidConfig(format!("MCP bridge call requires '{key}'")))
}

fn format_resources_listing(
    server_name: &str,
    page: &crate::McpListPage<McpResource>,
) -> McpToolCallResult {
    if page.items.is_empty() {
        let mut text = format!("No MCP resources are currently exposed by server '{server_name}'.");
        if let Some(cursor) = &page.next_cursor {
            text.push_str(&format!("\nNext cursor: {}", cursor.as_str()));
        }
        return McpToolCallResult::text(text);
    }

    let mut lines = vec![format!("MCP resources from server '{server_name}':")];
    for (idx, resource) in page.items.iter().enumerate() {
        lines.push(format!("{}. {}", idx + 1, resource.name));
        lines.push(format!("   URI: {}", resource.uri.as_str()));
        if let Some(title) = &resource.title {
            lines.push(format!("   Title: {title}"));
        }
        if let Some(description) = &resource.description {
            lines.push(format!("   Description: {description}"));
        }
        if let Some(mime_type) = &resource.mime_type {
            lines.push(format!("   MIME type: {}", mime_type.as_str()));
        }
        if let Some(size) = resource.size {
            lines.push(format!("   Size: {size} bytes"));
        }
    }
    if let Some(cursor) = &page.next_cursor {
        lines.push(format!("Next cursor: {}", cursor.as_str()));
    }
    McpToolCallResult::text(lines.join("\n"))
}

fn format_resource_templates_listing(
    server_name: &str,
    page: &crate::McpListPage<McpResourceTemplate>,
) -> McpToolCallResult {
    if page.items.is_empty() {
        let mut text =
            format!("No MCP resource templates are currently exposed by server '{server_name}'.");
        if let Some(cursor) = &page.next_cursor {
            text.push_str(&format!("\nNext cursor: {}", cursor.as_str()));
        }
        return McpToolCallResult::text(text);
    }

    let mut lines = vec![format!(
        "MCP resource templates from server '{server_name}':"
    )];
    for (idx, template) in page.items.iter().enumerate() {
        lines.push(format!("{}. {}", idx + 1, template.name));
        lines.push(format!("   URI template: {}", template.uri_template));
        if let Some(title) = &template.title {
            lines.push(format!("   Title: {title}"));
        }
        if let Some(description) = &template.description {
            lines.push(format!("   Description: {description}"));
        }
        if let Some(mime_type) = &template.mime_type {
            lines.push(format!("   MIME type: {}", mime_type.as_str()));
        }
    }
    if let Some(cursor) = &page.next_cursor {
        lines.push(format!("Next cursor: {}", cursor.as_str()));
    }
    McpToolCallResult::text(lines.join("\n"))
}

fn format_prompts_listing(
    server_name: &str,
    page: &crate::McpListPage<McpPrompt>,
) -> McpToolCallResult {
    if page.items.is_empty() {
        let mut text = format!("No MCP prompts are currently exposed by server '{server_name}'.");
        if let Some(cursor) = &page.next_cursor {
            text.push_str(&format!("\nNext cursor: {}", cursor.as_str()));
        }
        return McpToolCallResult::text(text);
    }

    let mut lines = vec![format!("MCP prompts from server '{server_name}':")];
    for (idx, prompt) in page.items.iter().enumerate() {
        lines.push(format!("{}. {}", idx + 1, prompt.name));
        if let Some(title) = &prompt.title {
            lines.push(format!("   Title: {title}"));
        }
        if let Some(description) = &prompt.description {
            lines.push(format!("   Description: {description}"));
        }
        if !prompt.arguments.is_empty() {
            lines.push("   Arguments:".to_string());
            for argument in &prompt.arguments {
                let suffix = if argument.required {
                    "required"
                } else {
                    "optional"
                };
                if let Some(description) = &argument.description {
                    lines.push(format!(
                        "   - {} ({}): {}",
                        argument.name, suffix, description
                    ));
                } else {
                    lines.push(format!("   - {} ({})", argument.name, suffix));
                }
            }
        }
    }
    if let Some(cursor) = &page.next_cursor {
        lines.push(format!("Next cursor: {}", cursor.as_str()));
    }
    McpToolCallResult::text(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use astrbot_tool::ToolSource;
    use serde_json::json;

    use super::{McpBridgeCall, McpBridgeCatalogBuilder};
    use crate::{
        McpJsonObject, McpJsonSchema, McpPrompt, McpResource, McpResourceTemplate, McpServerName,
        McpTool, McpUri,
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

    #[test]
    fn bridge_registration_resolves_catalog_descriptor_names_to_mcp_calls() {
        let builder =
            McpBridgeCatalogBuilder::new(McpServerName::new("Docs Server").expect("server name"));
        let tool = McpTool::new("Search Docs");
        let resource = McpResource::new(
            McpUri::new("file:///docs/readme.md").expect("uri"),
            "readme",
        );
        let prompt = McpPrompt::new("summarize");
        let registration = builder.build_registration(&[tool], &[resource], &[], &[prompt]);

        let call = registration
            .resolve_call(
                "mcp_docs_server_search_docs",
                McpJsonObject::new().with("query", "rust"),
            )
            .expect("tool call should resolve");
        let McpBridgeCall::Tool(request) = call else {
            panic!("expected MCP tool call");
        };
        assert_eq!(request.name, "Search Docs");
        assert_eq!(
            request
                .arguments
                .get("query")
                .and_then(|value| value.as_str()),
            Some("rust")
        );

        let call = registration
            .resolve_call(
                "mcp_docs_server_read_resource",
                McpJsonObject::new().with("uri", "file:///docs/readme.md"),
            )
            .expect("resource call should resolve");
        assert!(matches!(call, McpBridgeCall::ReadResource(_)));

        let call = registration
            .resolve_call(
                "mcp_docs_server_get_prompt",
                McpJsonObject::new().with("name", "summarize"),
            )
            .expect("prompt call should resolve");
        assert!(matches!(call, McpBridgeCall::GetPrompt(_)));
    }
}
