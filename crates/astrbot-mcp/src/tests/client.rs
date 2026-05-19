use std::{
    collections::VecDeque,
    fs,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::{
    McpClientBoundary, McpClientCapabilities, McpClientLifecycle, McpConcreteClient,
    McpContentBlock, McpCursor, McpError, McpGetPromptRequest, McpJsonRpcFrame,
    McpReadResourceRequest, McpReconnectPolicy, McpRootsCapabilityConfig, McpServerConfig,
    McpServerName, McpToolCallRequest, McpTransportEndpoint, McpTransportRuntime,
    McpTransportSession, McpUri,
};

#[tokio::test]
async fn concrete_client_uses_configured_transport_endpoints() {
    for (config, expected) in [
        (
            McpServerConfig::stdio("node").with_arg("server.js"),
            "stdio",
        ),
        (McpServerConfig::sse("https://example.test/sse"), "sse"),
        (
            McpServerConfig::streamable_http("https://example.test/mcp"),
            "streamable_http",
        ),
    ] {
        let runtime = Arc::new(FakeRuntime::new(vec![ok(json!({})), tools_result(vec![])]));
        let client = McpConcreteClient::new(runtime.clone());

        client
            .connect(server_name("docs"), config)
            .await
            .expect("connect should run");

        let endpoint = runtime
            .endpoints()
            .pop()
            .expect("endpoint should be recorded");
        assert_eq!(endpoint_kind(&endpoint), expected);
    }
}

#[tokio::test]
async fn concrete_client_lists_calls_resources_prompts_and_shutdowns() {
    let runtime = Arc::new(FakeRuntime::new(vec![
        ok(json!({})),
        tools_result(vec![
            json!({"name": "search", "description": "Search docs"}),
        ]),
        ok(json!({
            "content": [{"type": "text", "text": "tool result"}],
            "isError": false
        })),
        ok(json!({
            "resources": [{"uri": "file:///docs/readme.md", "name": "readme"}],
            "nextCursor": "r2"
        })),
        ok(json!({
            "contents": [{"type": "text", "uri": "file:///docs/readme.md", "text": "hello"}]
        })),
        ok(json!({
            "prompts": [{"name": "summarize"}],
            "nextCursor": "p2"
        })),
        ok(json!({
            "description": "Summarize docs",
            "messages": [{"role": "user", "content": {"type": "text", "text": "summarize"}}]
        })),
    ]));
    let client = McpConcreteClient::new(runtime.clone());
    client
        .connect(
            server_name("docs"),
            McpServerConfig::streamable_http("https://example.test/mcp"),
        )
        .await
        .expect("connect should run");

    let tool = client
        .call_tool(McpToolCallRequest::new("search").with_argument("query", "rust"))
        .await
        .expect("tool should run");
    assert_eq!(
        tool.content,
        vec![McpContentBlock::Text {
            text: "tool result".to_string()
        }]
    );

    let resources = client
        .list_resources(McpCursor::new("r1"))
        .await
        .expect("resources should list");
    assert_eq!(resources.items[0].name, "readme");
    assert_eq!(resources.next_cursor.expect("cursor").as_str(), "r2");

    let read = client
        .read_resource(McpReadResourceRequest::new(uri("file:///docs/readme.md")))
        .await
        .expect("resource should read");
    assert_eq!(read.contents.len(), 1);

    let prompts = client
        .list_prompts(None)
        .await
        .expect("prompts should list");
    assert_eq!(prompts.items[0].name, "summarize");

    let prompt = client
        .get_prompt(McpGetPromptRequest::new("summarize"))
        .await
        .expect("prompt should load");
    assert_eq!(prompt.description.as_deref(), Some("Summarize docs"));

    client.shutdown().await.expect("shutdown should close");
    assert_eq!(runtime.closed_sessions(), vec!["session-1".to_string()]);
    assert!(runtime.sent_methods().contains(&"shutdown".to_string()));

    let request_log = runtime.request_log();
    assert_eq!(request_log[2].method, "tools/call");
    assert_eq!(
        request_log[2].timeout,
        Duration::from_secs(crate::config::DEFAULT_MCP_SESSION_READ_TIMEOUT_SECONDS)
    );
}

#[tokio::test]
async fn concrete_client_preloads_resources_prompts_templates_from_server_capabilities() {
    let runtime = Arc::new(FakeRuntime::new(vec![
        ok(json!({
            "capabilities": {
                "tools": {},
                "resources": {"subscribe": false, "listChanged": true},
                "prompts": {"listChanged": true}
            }
        })),
        tools_result(vec![json!({"name": "search"})]),
        ok(json!({
            "resources": [{"uri": "file:///docs/readme.md", "name": "readme"}]
        })),
        ok(json!({
            "resourceTemplates": [{
                "uriTemplate": "file:///docs/{name}",
                "name": "doc"
            }]
        })),
        ok(json!({
            "prompts": [{"name": "summarize"}]
        })),
    ]));
    let client = McpConcreteClient::new(runtime.clone());

    client
        .connect(
            server_name("docs"),
            McpServerConfig::streamable_http("https://example.test/mcp"),
        )
        .await
        .expect("connect should run");

    let snapshot = client.snapshot();
    assert!(
        snapshot
            .server_capabilities
            .as_ref()
            .expect("capabilities")
            .supports_resources()
    );
    assert_eq!(snapshot.resources[0].name, "readme");
    assert_eq!(snapshot.resource_templates[0].name, "doc");
    assert_eq!(snapshot.prompts[0].name, "summarize");
    assert_eq!(
        runtime
            .request_log()
            .into_iter()
            .map(|request| request.method)
            .collect::<Vec<_>>(),
        vec![
            "initialize",
            "tools/list",
            "resources/list",
            "resources/templates/list",
            "prompts/list"
        ]
    );
}

#[tokio::test]
async fn concrete_client_lists_configured_roots_from_runtime_filesystem() {
    let root =
        std::env::temp_dir().join(format!("astrbot-mcp-client-roots-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("clean root");
    }
    fs::create_dir_all(root.join("data").join("temp")).expect("temp root");

    let runtime = Arc::new(FakeRuntime::new(vec![ok(json!({})), tools_result(vec![])]));
    let client = McpConcreteClient::new(runtime).with_root_base_path(&root);
    client
        .connect(
            server_name("docs"),
            McpServerConfig::stdio("node").with_client_capabilities(McpClientCapabilities {
                roots: McpRootsCapabilityConfig {
                    enabled: true,
                    paths: vec!["temp".to_string()],
                },
                ..McpClientCapabilities::default()
            }),
        )
        .await
        .expect("connect should run");

    let roots = client
        .list_roots(crate::McpRootsRequest::default())
        .await
        .expect("roots should resolve");
    assert_eq!(roots[0].name.as_deref(), Some("temp"));

    fs::remove_dir_all(root).expect("clean root");
}

#[tokio::test]
async fn concrete_client_reconnects_once_on_closed_resource_transport_error() {
    let runtime = Arc::new(FakeRuntime::new(vec![
        ok(json!({})),
        tools_result(vec![]),
        Err(McpError::Transport("closed resource".to_string())),
        ok(json!({})),
        tools_result(vec![]),
        ok(json!({"content": [{"type": "text", "text": "after reconnect"}]})),
    ]));
    let client =
        McpConcreteClient::new(runtime.clone()).with_reconnect_policy(McpReconnectPolicy {
            max_attempts: 2,
            backoff_initial_ms: 1,
            backoff_max_ms: 3,
        });
    client
        .connect(server_name("docs"), McpServerConfig::stdio("node"))
        .await
        .expect("connect should run");

    let result = client
        .call_tool(McpToolCallRequest::new("search"))
        .await
        .expect("tool should retry after reconnect");

    assert_eq!(
        result.content,
        vec![McpContentBlock::Text {
            text: "after reconnect".to_string()
        }]
    );
    assert_eq!(runtime.endpoints().len(), 2);
}

#[tokio::test]
async fn concrete_client_records_protocol_errors_and_honors_session_timeout() {
    let runtime = Arc::new(FakeRuntime::new(vec![
        ok(json!({})),
        tools_result(vec![]),
        err("server exploded"),
    ]));
    let client = McpConcreteClient::new(runtime.clone());
    client
        .connect(
            server_name("docs"),
            McpServerConfig {
                session_read_timeout_seconds: 7,
                ..McpServerConfig::stdio("node")
            },
        )
        .await
        .expect("connect should run");

    let error = client
        .call_tool(McpToolCallRequest::new("search"))
        .await
        .expect_err("protocol error should surface");

    assert_eq!(error, McpError::Protocol("server exploded".to_string()));
    assert_eq!(
        client.snapshot().server_errors,
        vec!["server exploded".to_string()]
    );
    assert_eq!(
        runtime.request_log().last().expect("request").timeout,
        Duration::from_secs(7)
    );
}

#[derive(Clone, Debug)]
struct RequestLog {
    method: String,
    timeout: Duration,
}

#[derive(Default)]
struct FakeRuntime {
    responses: Mutex<VecDeque<crate::McpResult<McpJsonRpcFrame>>>,
    endpoints: Mutex<Vec<McpTransportEndpoint>>,
    requests: Mutex<Vec<RequestLog>>,
    sent_methods: Mutex<Vec<String>>,
    closed_sessions: Mutex<Vec<String>>,
}

impl FakeRuntime {
    fn new(responses: Vec<crate::McpResult<McpJsonRpcFrame>>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
            ..Self::default()
        }
    }

    fn endpoints(&self) -> Vec<McpTransportEndpoint> {
        self.endpoints.lock().expect("endpoints").clone()
    }

    fn request_log(&self) -> Vec<RequestLog> {
        self.requests.lock().expect("requests").clone()
    }

    fn sent_methods(&self) -> Vec<String> {
        self.sent_methods.lock().expect("sent").clone()
    }

    fn closed_sessions(&self) -> Vec<String> {
        self.closed_sessions.lock().expect("closed").clone()
    }
}

#[async_trait]
impl McpTransportRuntime for FakeRuntime {
    async fn connect(
        &self,
        endpoint: McpTransportEndpoint,
    ) -> crate::McpResult<McpTransportSession> {
        let mut endpoints = self.endpoints.lock().expect("endpoints");
        let session_id = format!("session-{}", endpoints.len() + 1);
        endpoints.push(endpoint.clone());
        Ok(McpTransportSession {
            session_id,
            endpoint,
        })
    }

    async fn send(
        &self,
        _session: &McpTransportSession,
        frame: McpJsonRpcFrame,
    ) -> crate::McpResult<()> {
        if let Some(method) = frame.value.get("method").and_then(Value::as_str) {
            self.sent_methods
                .lock()
                .expect("sent")
                .push(method.to_string());
        }
        Ok(())
    }

    async fn request(
        &self,
        _session: &McpTransportSession,
        frame: McpJsonRpcFrame,
        read_timeout: Duration,
    ) -> crate::McpResult<McpJsonRpcFrame> {
        self.requests.lock().expect("requests").push(RequestLog {
            method: frame
                .value
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            timeout: read_timeout,
        });
        self.responses
            .lock()
            .expect("responses")
            .pop_front()
            .expect("fake response should exist")
    }

    async fn close(&self, session: McpTransportSession) -> crate::McpResult<()> {
        self.closed_sessions
            .lock()
            .expect("closed")
            .push(session.session_id);
        Ok(())
    }
}

fn ok(result: Value) -> crate::McpResult<McpJsonRpcFrame> {
    Ok(McpJsonRpcFrame {
        value: json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": result
        }),
    })
}

fn err(message: &str) -> crate::McpResult<McpJsonRpcFrame> {
    Ok(McpJsonRpcFrame {
        value: json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32000,
                "message": message
            }
        }),
    })
}

fn tools_result(tools: Vec<Value>) -> crate::McpResult<McpJsonRpcFrame> {
    ok(json!({ "tools": tools }))
}

fn server_name(name: &str) -> McpServerName {
    McpServerName::new(name).expect("server name")
}

fn uri(value: &str) -> McpUri {
    McpUri::new(value).expect("uri")
}

fn endpoint_kind(endpoint: &McpTransportEndpoint) -> &'static str {
    match endpoint {
        McpTransportEndpoint::Stdio { .. } => "stdio",
        McpTransportEndpoint::Sse { .. } => "sse",
        McpTransportEndpoint::StreamableHttp { .. } => "streamable_http",
    }
}
