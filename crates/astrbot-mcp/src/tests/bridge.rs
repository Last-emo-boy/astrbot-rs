use async_trait::async_trait;

use crate::{
    McpBridgeCatalogBuilder, McpClientBoundary, McpClientLifecycle, McpClientSnapshot,
    McpConnectionReport, McpCursor, McpGetPromptRequest, McpGetPromptResult, McpJsonObject,
    McpListPage, McpPrompt, McpPromptMessage, McpReadResourceRequest, McpReadResourceResult,
    McpReconnectPolicy, McpResource, McpResourceTemplate, McpRootsRequest, McpSamplingRequest,
    McpSamplingResult, McpServerConfig, McpServerName, McpTool, McpToolCallRequest,
    McpToolCallResult, McpUri, build_mcp_prompt_tool_names, build_mcp_resource_tool_names,
};

#[test]
fn bridge_tool_names_match_sanitized_server_scope() {
    assert_eq!(
        build_mcp_resource_tool_names("My MCP.Server", true),
        vec![
            "mcp_my_mcp_server_list_resources",
            "mcp_my_mcp_server_read_resource",
            "mcp_my_mcp_server_list_resource_templates"
        ]
    );
    assert_eq!(
        build_mcp_prompt_tool_names("My MCP.Server"),
        vec![
            "mcp_my_mcp_server_list_prompts",
            "mcp_my_mcp_server_get_prompt"
        ]
    );
}

#[tokio::test]
async fn bridge_executes_synthetic_resource_and_prompt_tools_against_client_boundary() {
    let server_name = McpServerName::new("Docs Server").expect("server name");
    let resource = McpResource::new(uri("file:///docs/readme.md"), "readme");
    let template = McpResourceTemplate {
        uri_template: "file:///docs/{name}".to_string(),
        name: "doc".to_string(),
        title: None,
        description: None,
        mime_type: None,
    };
    let prompt = McpPrompt::new("summarize");
    let registration = McpBridgeCatalogBuilder::new(server_name).build_registration(
        &[],
        &[resource],
        &[template],
        &[prompt],
    );
    let fake = FakeClient;

    let list = registration
        .execute_call(
            &fake,
            "mcp_docs_server_list_resources",
            McpJsonObject::new(),
        )
        .await
        .expect("list resources should execute");
    assert_text_contains(&list, "MCP resources from server 'Docs Server':");

    let read = registration
        .execute_call(
            &fake,
            "mcp_docs_server_read_resource",
            McpJsonObject::new().with("uri", "file:///docs/readme.md"),
        )
        .await
        .expect("read resource should execute");
    assert_text_contains(&read, "hello");

    let prompt = registration
        .execute_call(
            &fake,
            "mcp_docs_server_get_prompt",
            McpJsonObject::new().with("name", "summarize"),
        )
        .await
        .expect("get prompt should execute");
    assert_text_contains(&prompt, "MCP prompt from server 'Docs Server':");
}

#[test]
fn bridge_registers_synthetic_tools_from_capabilities_even_when_preload_is_empty() {
    let registration =
        McpBridgeCatalogBuilder::new(McpServerName::new("Docs Server").expect("server name"))
            .build_registration_with_capabilities(&[], &[], &[], &[], true, true);
    let names = registration
        .descriptors
        .iter()
        .map(|descriptor| descriptor.name.as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"mcp_docs_server_list_resources"));
    assert!(names.contains(&"mcp_docs_server_read_resource"));
    assert!(names.contains(&"mcp_docs_server_list_prompts"));
    assert!(names.contains(&"mcp_docs_server_get_prompt"));
    assert!(!names.contains(&"mcp_docs_server_list_resource_templates"));
}

struct FakeClient;

#[async_trait]
impl McpClientLifecycle for FakeClient {
    async fn connect(
        &self,
        server_name: McpServerName,
        _config: McpServerConfig,
    ) -> crate::McpResult<McpConnectionReport> {
        Ok(McpConnectionReport::ready(server_name))
    }

    async fn reconnect(
        &self,
        _policy: McpReconnectPolicy,
    ) -> crate::McpResult<McpConnectionReport> {
        Ok(McpConnectionReport::ready(
            McpServerName::new("fake").expect("name"),
        ))
    }

    async fn shutdown(&self) -> crate::McpResult<()> {
        Ok(())
    }

    fn snapshot(&self) -> McpClientSnapshot {
        McpClientSnapshot::default()
    }
}

#[async_trait]
impl McpClientBoundary for FakeClient {
    async fn list_tools(
        &self,
        _cursor: Option<McpCursor>,
    ) -> crate::McpResult<McpListPage<McpTool>> {
        Ok(McpListPage::new(Vec::new()))
    }

    async fn call_tool(&self, request: McpToolCallRequest) -> crate::McpResult<McpToolCallResult> {
        Ok(McpToolCallResult::text(format!("tool {}", request.name)))
    }

    async fn list_resources(
        &self,
        _cursor: Option<McpCursor>,
    ) -> crate::McpResult<McpListPage<McpResource>> {
        Ok(McpListPage::new(vec![McpResource::new(
            uri("file:///docs/readme.md"),
            "readme",
        )]))
    }

    async fn list_resource_templates(
        &self,
        _cursor: Option<McpCursor>,
    ) -> crate::McpResult<McpListPage<McpResourceTemplate>> {
        Ok(McpListPage::new(vec![McpResourceTemplate {
            uri_template: "file:///docs/{name}".to_string(),
            name: "doc".to_string(),
            title: None,
            description: None,
            mime_type: None,
        }]))
    }

    async fn read_resource(
        &self,
        request: McpReadResourceRequest,
    ) -> crate::McpResult<McpReadResourceResult> {
        Ok(McpReadResourceResult::text(request.uri, "hello"))
    }

    async fn list_prompts(
        &self,
        _cursor: Option<McpCursor>,
    ) -> crate::McpResult<McpListPage<McpPrompt>> {
        Ok(McpListPage::new(vec![McpPrompt::new("summarize")]))
    }

    async fn get_prompt(
        &self,
        request: McpGetPromptRequest,
    ) -> crate::McpResult<McpGetPromptResult> {
        Ok(McpGetPromptResult {
            description: Some(format!("Prompt {}", request.name)),
            messages: vec![McpPromptMessage {
                role: crate::McpSamplingRole::User,
                content: crate::McpContentBlock::Text {
                    text: "summarize".to_string(),
                },
            }],
        })
    }

    async fn create_sampling_message(
        &self,
        _request: McpSamplingRequest,
    ) -> crate::McpResult<McpSamplingResult> {
        Ok(McpSamplingResult::assistant_text("ok", "fake"))
    }

    async fn elicit(
        &self,
        _request: crate::McpElicitationRequest,
    ) -> crate::McpResult<crate::McpElicitationResult> {
        Ok(crate::McpElicitationResult::decline())
    }

    async fn list_roots(&self, _request: McpRootsRequest) -> crate::McpResult<Vec<crate::McpRoot>> {
        Ok(Vec::new())
    }
}

fn assert_text_contains(result: &McpToolCallResult, needle: &str) {
    let text = result
        .content
        .iter()
        .filter_map(|block| match block {
            crate::McpContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains(needle),
        "expected `{text}` to contain `{needle}`"
    );
}

fn uri(value: &str) -> McpUri {
    McpUri::new(value).expect("uri")
}
