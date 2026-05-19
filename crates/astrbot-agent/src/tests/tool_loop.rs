use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use astrbot_computer::{
    BooterKind, BooterSession, ComputerComponent, ComputerRuntimeConfig, ComputerUseRuntime,
    ComputerUseSession, DOWNLOAD_FILE_TOOL, EXECUTE_SHELL_TOOL, RUN_BROWSER_SKILL_TOOL,
    RecordingComputerRuntimePort,
};
use astrbot_core::{
    AstrbotError, MessageEvent, ProviderContentPart, ProviderContextMessage, Result,
};
use astrbot_kb::{
    ChunkId, DocumentId, EmbeddedKnowledgeChunk, HybridKnowledgeRetriever, InMemoryVectorStore,
    KnowledgeBaseId, KnowledgeChunk, VectorStore, VectorStoreSparseRetriever,
};
use astrbot_mcp::{
    McpBridgeCall, McpBridgeCatalogBuilder, McpBridgeRegistration, McpContentBlock, McpJsonObject,
    McpJsonSchema, McpJsonValue, McpServerName, McpTool, McpToolCallRequest, McpToolCallResult,
};
use astrbot_plugin::{
    PluginContext, PluginToolDeclaration, ToolExecutionRequest as PluginToolExecutionRequest,
    ToolExecutionResult as PluginToolExecutionResult, ToolExecutor as PluginToolExecutor,
};
use astrbot_provider::{
    ChatProvider, ChatRequest, ChatResponse, MockEmbeddingProvider, ProviderResponseMetadata,
    ProviderToolCall, ProviderToolCallArguments,
};
use astrbot_tool::{
    BAIDU_AI_SEARCH_TOOL, FETCH_URL_TOOL, ToolCatalog, ToolDescriptor, ToolSource,
    WEB_SEARCH_BOCHA_TOOL, WEB_SEARCH_TAVILY_TOOL, WEB_SEARCH_TOOL, WebSearchProvider,
    WebSearchSessionConfig, builtin_internal_tool_catalog,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::{
    AgentFeedbackEventKind, AgentHookEventKind, AgentKnowledgeContextSelection, AgentRunner,
    AgentStopSignal, AgentToolExecutionRequest, AgentToolExecutionResult, AgentToolExecutor,
    ComputerUseToolCatalogFilter, ComputerUseToolExecutor, InMemoryToolImageCache,
    KnowledgeContextRequestDecorator, KnowledgeRetrievalContextService,
    KnowledgeSearchToolExecutor, StaticComputerUseSessionPort, StaticStopSignalPort,
    ToolLoopAgentRunner, ToolLoopPolicy, WebSearchSessionConfigPort, WebSearchToolCatalogFilter,
    arguments_from_json,
};

use super::support::{CapturingHook, StaticKnowledgeSelection, event};

#[test]
fn tool_loop_policy_normalizes_limits() {
    let policy = ToolLoopPolicy::default()
        .enabled()
        .with_max_steps(0)
        .with_timeout_seconds(0)
        .with_schema_mode("skills-like");

    assert_eq!(policy.max_steps, 1);
    assert_eq!(policy.tool_call_timeout_seconds, 1);
    assert_eq!(policy.schema_mode, "skills-like");
}

#[tokio::test]
async fn tool_loop_runner_executes_multi_tool_calls_and_records_context_hooks() {
    let provider = Arc::new(ScriptedProvider::new(vec![
        tool_response(vec![
            tool_call(
                "call-1",
                "search",
                json!({"q": "rust", "ignored": "drop me"}),
            ),
            tool_call("call-2", "calc", json!({"expr": "1+1"})),
        ]),
        ChatResponse::text("final answer"),
    ]));
    let executor = Arc::new(ScriptedToolExecutor::new(vec![
        ToolAction::Ok(AgentToolExecutionResult::completed("search result")),
        ToolAction::Ok(AgentToolExecutionResult::completed("2")),
    ]));
    let hook = Arc::new(CapturingHook::default());
    let runner = ToolLoopAgentRunner::new(provider.clone(), catalog(), executor.clone())
        .with_hook(hook.clone());

    let outcome = runner
        .run(&event("hello"))
        .await
        .expect("tool loop should run");

    assert_eq!(
        outcome.result().expect("final result").chain.plain_text(),
        "final answer"
    );
    let provider_requests = provider.requests();
    assert_eq!(provider_requests.len(), 2);
    assert_eq!(provider_requests[0].tool_placeholders.len(), 3);
    assert!(context_contains(
        &provider_requests[1],
        "tool_call id=call-1 name=search"
    ));
    assert!(context_contains(&provider_requests[1], "search result"));
    assert!(context_contains(&provider_requests[1], "2"));

    let tool_requests = executor.requests();
    assert_eq!(tool_requests.len(), 2);
    assert_eq!(tool_requests[0].descriptor.name, "search");
    assert_eq!(tool_requests[0].argument("q"), Some(&json!("rust")));
    assert!(tool_requests[0].argument("ignored").is_none());
    assert_eq!(tool_requests[1].descriptor.name, "calc");
    assert_eq!(
        hook.kinds(),
        vec![
            AgentHookEventKind::AgentBegin,
            AgentHookEventKind::ToolStart,
            AgentHookEventKind::ToolEnd,
            AgentHookEventKind::ToolStart,
            AgentHookEventKind::ToolEnd,
            AgentHookEventKind::AgentDone,
        ]
    );
    assert_eq!(
        outcome
            .feedback_events()
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        vec![
            AgentFeedbackEventKind::ToolCall,
            AgentFeedbackEventKind::ToolResult,
            AgentFeedbackEventKind::ToolCall,
            AgentFeedbackEventKind::ToolResult,
        ]
    );
}

#[tokio::test]
async fn tool_loop_runner_reports_missing_tool_and_executor_failure() {
    let provider = Arc::new(ScriptedProvider::new(vec![
        tool_response(vec![
            tool_call("call-1", "missing", json!({})),
            tool_call("call-2", "search", json!({"q": "rust"})),
        ]),
        ChatResponse::text("handled error"),
    ]));
    let executor = Arc::new(ScriptedToolExecutor::new(vec![ToolAction::Err(
        "executor unavailable",
    )]));
    let runner = ToolLoopAgentRunner::new(provider.clone(), catalog(), executor);

    let outcome = runner
        .run(&event("hello"))
        .await
        .expect("tool loop should recover tool errors");

    assert_eq!(
        outcome.result().expect("final result").chain.plain_text(),
        "handled error"
    );
    let provider_requests = provider.requests();
    assert!(context_contains(
        &provider_requests[1],
        "error: Tool missing not found."
    ));
    assert!(context_contains(
        &provider_requests[1],
        "pipeline error: executor unavailable"
    ));
}

#[tokio::test]
async fn tool_loop_runner_forces_final_response_at_max_steps() {
    let provider = Arc::new(ScriptedProvider::new(vec![
        tool_response(vec![tool_call("call-1", "search", json!({"q": "rust"}))]),
        ChatResponse::text("forced summary"),
    ]));
    let executor = Arc::new(ScriptedToolExecutor::new(vec![ToolAction::Ok(
        AgentToolExecutionResult::completed("search result"),
    )]));
    let runner = ToolLoopAgentRunner::new(provider.clone(), catalog(), executor)
        .with_policy(ToolLoopPolicy::default().enabled().with_max_steps(1));

    let outcome = runner
        .run(&event("hello"))
        .await
        .expect("tool loop should force final response");

    assert_eq!(
        outcome.result().expect("final result").chain.plain_text(),
        "forced summary"
    );
    let provider_requests = provider.requests();
    assert_eq!(provider_requests.len(), 2);
    assert!(provider_requests[1].tool_placeholders.is_empty());
    assert!(context_contains(
        &provider_requests[1],
        "工具调用次数已达到上限"
    ));
}

#[tokio::test]
async fn tool_loop_runner_stops_when_stop_signal_requests_abort() {
    let provider = Arc::new(ScriptedProvider::new(vec![ChatResponse::text(
        "should not be called",
    )]));
    let executor = Arc::new(ScriptedToolExecutor::new(Vec::new()));
    let runner =
        ToolLoopAgentRunner::new(provider.clone(), catalog(), executor).with_stop_signal(Arc::new(
            StaticStopSignalPort::new(AgentStopSignal::default().user_requested()),
        ));

    let outcome = runner
        .run(&event("hello"))
        .await
        .expect("stop signal should be handled");

    assert!(outcome.result().is_none());
    assert!(provider.requests().is_empty());
    assert_eq!(
        outcome.feedback_events().last().map(|event| event.kind),
        Some(AgentFeedbackEventKind::Aborted)
    );
}

#[tokio::test]
async fn tool_loop_runner_caches_image_result_for_provider_review() {
    let provider = Arc::new(ScriptedProvider::new(vec![
        tool_response(vec![tool_call("call-1", "draw", json!({"prompt": "cat"}))]),
        ChatResponse::text("image reviewed"),
    ]));
    let executor = Arc::new(ScriptedToolExecutor::new(vec![ToolAction::Ok(
        AgentToolExecutionResult::image("aW1hZ2U=", "image/png"),
    )]));
    let cache = Arc::new(InMemoryToolImageCache::new(Duration::from_secs(60)));
    let runner = ToolLoopAgentRunner::new(provider.clone(), catalog(), executor)
        .with_image_cache(cache.clone());

    let outcome = runner
        .run(&event("hello"))
        .await
        .expect("tool loop should pass cached image to provider");

    assert_eq!(
        outcome.result().expect("final result").chain.plain_text(),
        "image reviewed"
    );
    assert_eq!(cache.len(), 1);
    let provider_requests = provider.requests();
    assert!(context_has_image_data_url(&provider_requests[1]));
}

#[tokio::test]
async fn tool_loop_runner_requeries_arguments_in_skills_like_mode() {
    let provider = Arc::new(ScriptedProvider::new(vec![
        tool_response(vec![ProviderToolCall::new(
            "call-1",
            "search",
            ProviderToolCallArguments::Empty,
        )]),
        tool_response(vec![tool_call("call-1", "search", json!({"q": "rust"}))]),
        ChatResponse::text("final after requery"),
    ]));
    let executor = Arc::new(ScriptedToolExecutor::new(vec![ToolAction::Ok(
        AgentToolExecutionResult::completed("search result"),
    )]));
    let runner = ToolLoopAgentRunner::new(provider.clone(), catalog(), executor.clone())
        .with_policy(
            ToolLoopPolicy::default()
                .enabled()
                .with_schema_mode("skills_like"),
        );

    let outcome = runner
        .run(&event("hello"))
        .await
        .expect("skills-like requery should run");

    assert_eq!(
        outcome.result().expect("final result").chain.plain_text(),
        "final after requery"
    );
    assert_eq!(provider.requests().len(), 3);
    assert_eq!(
        executor.requests()[0].argument("q"),
        Some(&Value::String("rust".to_string()))
    );
}

#[tokio::test]
async fn tool_loop_runner_executes_internal_kb_search_tool() {
    let vector_store = Arc::new(InMemoryVectorStore::default());
    let kb_id = KnowledgeBaseId::new("kb-1").expect("kb id");
    vector_store
        .upsert_chunks(vec![EmbeddedKnowledgeChunk::new(
            KnowledgeChunk::new(
                ChunkId::new("chunk-1").expect("chunk id"),
                kb_id.clone(),
                DocumentId::new("doc-1").expect("doc id"),
                0,
                "Knowledge base tool-loop result.",
            )
            .with_metadata("kb_name", json!("Docs"))
            .with_metadata("doc_name", json!("tool.md")),
            vec![1.0, 0.0],
        )])
        .await
        .expect("vectors should seed");
    let retriever = Arc::new(HybridKnowledgeRetriever::new(
        vector_store.clone(),
        Arc::new(VectorStoreSparseRetriever::new(vector_store)),
    ));
    let context = KnowledgeRetrievalContextService::new(
        Arc::new(StaticKnowledgeSelection::new(
            AgentKnowledgeContextSelection::new(["kb-1"])
                .with_top_k(1)
                .with_embedding_provider_id("embedding"),
        )),
        Arc::new(MockEmbeddingProvider::new(vec![1.0, 0.0])),
        retriever,
    );
    let executor = Arc::new(KnowledgeSearchToolExecutor::new(context));
    let provider = Arc::new(ScriptedProvider::new(vec![
        tool_response(vec![tool_call(
            "call-1",
            "astr_kb_search",
            json!({"query": "tool-loop"}),
        )]),
        ChatResponse::text("final with kb"),
    ]));
    let runner = ToolLoopAgentRunner::new(
        provider.clone(),
        builtin_internal_tool_catalog().into_tool_catalog(),
        executor,
    );

    let outcome = runner
        .run(&event("find knowledge"))
        .await
        .expect("kb tool should run through tool-loop");

    assert_eq!(
        outcome.result().expect("final result").chain.plain_text(),
        "final with kb"
    );
    let provider_requests = provider.requests();
    assert!(context_contains(
        &provider_requests[1],
        "Knowledge base tool-loop result."
    ));
    assert!(context_contains(
        &provider_requests[1],
        "来源: Docs / tool.md"
    ));
}

#[tokio::test]
async fn tool_loop_runner_executes_mcp_bridge_and_plugin_tool_in_same_loop() {
    let mcp_registration =
        McpBridgeCatalogBuilder::new(McpServerName::new("Docs Server").expect("server name"))
            .build_registration(
                &[McpTool::new("Search Docs")
                    .with_description("Search the docs MCP server")
                    .with_input_schema(McpJsonSchema::from_json(json!({
                        "type": "object",
                        "properties": {
                            "query": {"type": "string"}
                        },
                        "required": ["query"]
                    })))],
                &[],
                &[],
                &[],
            );
    let mcp_tool_name = mcp_registration.descriptors[0].name.clone();
    let plugin_declaration =
        PluginToolDeclaration::local("plugin_lookup").with_description("Plugin lookup");
    let mut tool_catalog = mcp_registration.clone().into_catalog();
    tool_catalog.add_tool(
        ToolDescriptor::new("plugin_lookup")
            .with_description("Plugin lookup")
            .with_parameters(json!({
                "type": "object",
                "properties": {
                    "topic": {"type": "string"}
                },
                "required": ["topic"]
            }))
            .with_source_metadata(
                plugin_declaration.source_metadata("plugin.lookup", "Lookup Plugin"),
            ),
    );

    let provider = Arc::new(ScriptedProvider::new(vec![
        tool_response(vec![
            tool_call("call-mcp", &mcp_tool_name, json!({"query": "rust"})),
            tool_call("call-plugin", "plugin_lookup", json!({"topic": "runtime"})),
        ]),
        ChatResponse::text("final with mcp and plugin"),
    ]));
    let mcp_server = Arc::new(RecordingMcpFakeServer::default());
    let plugin_executor = Arc::new(RecordingPluginToolExecutor::default());
    let executor = Arc::new(BridgeAndPluginToolExecutor {
        mcp_registration,
        mcp_server: mcp_server.clone(),
        plugin_declaration,
        plugin_executor: plugin_executor.clone(),
    });
    let runner = ToolLoopAgentRunner::new(provider.clone(), tool_catalog, executor);

    let outcome = runner
        .run(&event("look up docs and plugin data"))
        .await
        .expect("mcp and plugin tools should run through one tool loop");

    assert_eq!(
        outcome.result().expect("final result").chain.plain_text(),
        "final with mcp and plugin"
    );
    let provider_requests = provider.requests();
    assert_eq!(provider_requests.len(), 2);
    let initial_tools = provider_requests[0]
        .tool_placeholders
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(initial_tools.contains(&mcp_tool_name.as_str()));
    assert!(initial_tools.contains(&"plugin_lookup"));
    assert!(context_contains(
        &provider_requests[1],
        "mcp fake server result: rust"
    ));
    assert!(context_contains(
        &provider_requests[1],
        "plugin tool result: runtime"
    ));
    assert_eq!(mcp_server.calls(), vec!["Search Docs:rust"]);
    assert_eq!(
        plugin_executor
            .requests()
            .iter()
            .map(|request| {
                format!(
                    "{}:{}",
                    request.declaration.name,
                    request.argument("topic").unwrap_or_default()
                )
            })
            .collect::<Vec<_>>(),
        vec!["plugin_lookup:runtime"]
    );
}

#[tokio::test]
async fn tool_loop_runner_deduplicates_decorated_kb_context_when_search_tool_returns_same_text() {
    let vector_store = Arc::new(InMemoryVectorStore::default());
    let kb_id = KnowledgeBaseId::new("kb-1").expect("kb id");
    vector_store
        .upsert_chunks(vec![EmbeddedKnowledgeChunk::new(
            KnowledgeChunk::new(
                ChunkId::new("chunk-1").expect("chunk id"),
                kb_id.clone(),
                DocumentId::new("doc-1").expect("doc id"),
                0,
                "Shared KB context appears once.",
            )
            .with_metadata("kb_name", json!("Docs"))
            .with_metadata("doc_name", json!("shared.md")),
            vec![1.0, 0.0],
        )])
        .await
        .expect("vectors should seed");
    let retriever = Arc::new(HybridKnowledgeRetriever::new(
        vector_store.clone(),
        Arc::new(VectorStoreSparseRetriever::new(vector_store)),
    ));
    let context = KnowledgeRetrievalContextService::new(
        Arc::new(StaticKnowledgeSelection::new(
            AgentKnowledgeContextSelection::new(["kb-1"])
                .with_top_k(1)
                .with_embedding_provider_id("embedding"),
        )),
        Arc::new(MockEmbeddingProvider::new(vec![1.0, 0.0])),
        retriever,
    );
    let provider = Arc::new(ScriptedProvider::new(vec![
        tool_response(vec![tool_call(
            "call-kb",
            "astr_kb_search",
            json!({"query": "Shared KB context"}),
        )]),
        ChatResponse::text("final with deduped kb"),
    ]));
    let runner = ToolLoopAgentRunner::new(
        provider.clone(),
        builtin_internal_tool_catalog().into_tool_catalog(),
        Arc::new(KnowledgeSearchToolExecutor::new(context.clone())),
    )
    .with_request_decorator(Arc::new(KnowledgeContextRequestDecorator::new(Arc::new(
        context,
    ))));

    let outcome = runner
        .run(&event("Shared KB context"))
        .await
        .expect("kb decorator and tool search should coexist");

    assert_eq!(
        outcome.result().expect("final result").chain.plain_text(),
        "final with deduped kb"
    );
    let provider_requests = provider.requests();
    assert_eq!(provider_requests.len(), 2);
    assert_eq!(
        request_text_occurrences(&provider_requests[1], "Shared KB context appears once."),
        1
    );
    assert!(
        provider_requests[1]
            .system_prompt
            .as_deref()
            .expect("decorated knowledge context should exist")
            .contains("Shared KB context appears once.")
    );
    assert!(
        provider_requests[1].tool_call_results[0]
            .content
            .contains("Shared KB context appears once.")
    );
}

#[tokio::test]
async fn tool_loop_runner_filters_web_search_tools_from_session_config() {
    let provider = Arc::new(ScriptedProvider::new(vec![ChatResponse::text("final")]));
    let executor = Arc::new(ScriptedToolExecutor::new(Vec::new()));
    let runner = ToolLoopAgentRunner::new(provider.clone(), web_catalog(), executor)
        .with_catalog_filter(Arc::new(WebSearchToolCatalogFilter::new(Arc::new(
            StaticWebSearchConfig(WebSearchSessionConfig::enabled(WebSearchProvider::Tavily)),
        ))));

    runner
        .run(&event("hello"))
        .await
        .expect("tool loop should run");

    let requests = provider.requests();
    let placeholders = requests[0]
        .tool_placeholders
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(placeholders.contains(&WEB_SEARCH_TAVILY_TOOL));
    assert!(!placeholders.contains(&WEB_SEARCH_TOOL));
    assert!(!placeholders.contains(&FETCH_URL_TOOL));
    assert!(!placeholders.contains(&WEB_SEARCH_BOCHA_TOOL));
    assert!(placeholders.contains(&"search"));
}

#[tokio::test]
async fn tool_loop_runner_selects_baidu_ai_search_only_when_key_exists() {
    let provider = Arc::new(ScriptedProvider::new(vec![
        ChatResponse::text("missing-key"),
        ChatResponse::text("with-key"),
    ]));
    let executor = Arc::new(ScriptedToolExecutor::new(Vec::new()));
    let config_source = Arc::new(QueueWebSearchConfig::new(vec![
        WebSearchSessionConfig::enabled(WebSearchProvider::BaiduAiSearch),
        WebSearchSessionConfig::enabled(WebSearchProvider::BaiduAiSearch)
            .with_baidu_app_builder_key("app-key"),
    ]));
    let runner = ToolLoopAgentRunner::new(provider.clone(), web_catalog(), executor)
        .with_catalog_filter(Arc::new(WebSearchToolCatalogFilter::new(config_source)));

    runner
        .run(&event("first"))
        .await
        .expect("missing key run should finish");
    runner
        .run(&event("second"))
        .await
        .expect("with key run should finish");

    let requests = provider.requests();
    let first = requests[0]
        .tool_placeholders
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    let second = requests[1]
        .tool_placeholders
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(!first.contains(&BAIDU_AI_SEARCH_TOOL));
    assert!(second.contains(&BAIDU_AI_SEARCH_TOOL));
    assert!(!second.contains(&WEB_SEARCH_TOOL));
    assert!(!second.contains(&WEB_SEARCH_TAVILY_TOOL));
}

#[tokio::test]
async fn tool_loop_runner_injects_and_executes_computer_tools_by_runtime() {
    let runtime_port = Arc::new(RecordingComputerRuntimePort::new());
    let runtime = Arc::new(ComputerUseRuntime::new(runtime_port.clone()));
    let session_port = Arc::new(StaticComputerUseSessionPort::new(
        ComputerUseSession::local("conversation-1").with_admin(true),
    ));
    let provider = Arc::new(ScriptedProvider::new(vec![
        tool_response(vec![tool_call(
            "call-1",
            EXECUTE_SHELL_TOOL,
            json!({ "command": "echo ok" }),
        )]),
        ChatResponse::text("done"),
    ]));
    let runner = ToolLoopAgentRunner::new(
        provider.clone(),
        builtin_internal_tool_catalog().into_tool_catalog(),
        Arc::new(ComputerUseToolExecutor::new(
            runtime.clone(),
            session_port.clone(),
        )),
    )
    .with_catalog_filter(Arc::new(ComputerUseToolCatalogFilter::new(
        runtime,
        session_port,
    )))
    .with_policy(ToolLoopPolicy::default().enabled().with_max_steps(1));

    let outcome = runner
        .run(&event("run shell"))
        .await
        .expect("computer tool loop should execute");

    assert_eq!(
        outcome.result().expect("final result").chain.plain_text(),
        "done"
    );
    let requests = provider.requests();
    let tool_names = requests[0]
        .tool_placeholders
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&EXECUTE_SHELL_TOOL));
    assert!(!tool_names.contains(&DOWNLOAD_FILE_TOOL));
    assert_eq!(runtime_port.calls()[0].tool_name, EXECUTE_SHELL_TOOL);
}

#[tokio::test]
async fn computer_tool_executor_reports_permission_denied_and_runs_browser_skill() {
    let runtime_port = Arc::new(RecordingComputerRuntimePort::new());
    let runtime = Arc::new(ComputerUseRuntime::new(runtime_port.clone()));
    let denied_executor = ComputerUseToolExecutor::new(
        runtime.clone(),
        Arc::new(StaticComputerUseSessionPort::new(
            ComputerUseSession::local("conversation-1"),
        )),
    );
    let shell_descriptor = builtin_internal_tool_catalog()
        .into_tool_catalog()
        .tool(EXECUTE_SHELL_TOOL)
        .expect("shell descriptor")
        .clone();
    let denied = denied_executor
        .execute(AgentToolExecutionRequest::new(
            shell_descriptor,
            "call-denied",
            "conversation-1",
            arguments_from_json(&json!({ "command": "whoami" })),
            "{}",
        ))
        .await
        .expect("permission denial should be a tool result");
    assert!(denied.into_text().contains("administrator"));

    let sandbox_session = ComputerUseSession::sandbox(
        "conversation-1",
        ComputerRuntimeConfig::sandbox(BooterKind::ShipyardNeo),
    )
    .with_admin(true);
    let booter_session = BooterSession::new(
        "conversation-1",
        ComputerRuntimeConfig::sandbox(BooterKind::ShipyardNeo)
            .with_components([ComputerComponent::Browser, ComputerComponent::FileSystem]),
    );
    let sandbox_executor = ComputerUseToolExecutor::new(
        runtime,
        Arc::new(
            StaticComputerUseSessionPort::new(sandbox_session).with_booter_session(booter_session),
        ),
    );
    let skill_descriptor = builtin_internal_tool_catalog()
        .into_tool_catalog()
        .tool(RUN_BROWSER_SKILL_TOOL)
        .expect("browser skill descriptor")
        .clone();
    let result = sandbox_executor
        .execute(AgentToolExecutionRequest::new(
            skill_descriptor,
            "call-skill",
            "conversation-1",
            arguments_from_json(&json!({ "skill_key": "login" })),
            "{}",
        ))
        .await
        .expect("browser skill should execute");

    assert!(result.into_text().contains(RUN_BROWSER_SKILL_TOOL));
    assert_eq!(runtime_port.calls()[0].tool_name, RUN_BROWSER_SKILL_TOOL);
}

struct ScriptedProvider {
    responses: Mutex<VecDeque<ChatResponse>>,
    requests: Mutex<Vec<ChatRequest>>,
}

impl ScriptedProvider {
    fn new(responses: Vec<ChatResponse>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<ChatRequest> {
        self.requests
            .lock()
            .expect("provider requests should lock")
            .clone()
    }
}

#[async_trait]
impl ChatProvider for ScriptedProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        self.requests
            .lock()
            .expect("provider requests should lock")
            .push(request);
        Ok(self
            .responses
            .lock()
            .expect("provider responses should lock")
            .pop_front()
            .unwrap_or_else(|| ChatResponse::text("default final")))
    }
}

enum ToolAction {
    Ok(AgentToolExecutionResult),
    Err(&'static str),
}

struct ScriptedToolExecutor {
    actions: Mutex<VecDeque<ToolAction>>,
    requests: Mutex<Vec<AgentToolExecutionRequest>>,
}

impl ScriptedToolExecutor {
    fn new(actions: Vec<ToolAction>) -> Self {
        Self {
            actions: Mutex::new(VecDeque::from(actions)),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<AgentToolExecutionRequest> {
        self.requests
            .lock()
            .expect("tool requests should lock")
            .clone()
    }
}

#[async_trait]
impl AgentToolExecutor for ScriptedToolExecutor {
    async fn execute(
        &self,
        request: AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        self.requests
            .lock()
            .expect("tool requests should lock")
            .push(request);
        match self
            .actions
            .lock()
            .expect("tool actions should lock")
            .pop_front()
            .expect("scripted tool action should exist")
        {
            ToolAction::Ok(result) => Ok(result),
            ToolAction::Err(message) => Err(AstrbotError::Pipeline(message.to_string())),
        }
    }
}

struct BridgeAndPluginToolExecutor {
    mcp_registration: McpBridgeRegistration,
    mcp_server: Arc<RecordingMcpFakeServer>,
    plugin_declaration: PluginToolDeclaration,
    plugin_executor: Arc<RecordingPluginToolExecutor>,
}

#[async_trait]
impl AgentToolExecutor for BridgeAndPluginToolExecutor {
    async fn execute(
        &self,
        request: AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        match request.descriptor.source.kind {
            ToolSource::Mcp => self.execute_mcp(request).await,
            ToolSource::Plugin => self.execute_plugin(request).await,
            source => Err(AstrbotError::Pipeline(format!(
                "unexpected tool source for cross-crate fixture: {source:?}"
            ))),
        }
    }
}

impl BridgeAndPluginToolExecutor {
    async fn execute_mcp(
        &self,
        request: AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        let call = self
            .mcp_registration
            .resolve_call(
                &request.descriptor.name,
                mcp_arguments_from_json(&request.arguments)?,
            )
            .map_err(|err| AstrbotError::Pipeline(format!("mcp bridge route failed: {err}")))?;
        let McpBridgeCall::Tool(call) = call else {
            return Err(AstrbotError::Pipeline(
                "expected MCP bridge tool call route".to_string(),
            ));
        };
        Ok(AgentToolExecutionResult::completed(mcp_result_text(
            self.mcp_server.call(call),
        )))
    }

    async fn execute_plugin(
        &self,
        request: AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        let mut plugin_request = PluginToolExecutionRequest::new(
            self.plugin_declaration.clone(),
            PluginContext::new("plugin.lookup").with_session_id(request.session_id),
        );
        for (key, value) in request.arguments {
            plugin_request = plugin_request.with_argument(key, plugin_argument_string(value));
        }
        let result = self.plugin_executor.execute(plugin_request).await?;
        Ok(AgentToolExecutionResult::completed(
            result.content.unwrap_or_else(|| {
                "The plugin tool has no return value, or has sent the result directly to the user."
                    .to_string()
            }),
        ))
    }
}

#[derive(Default)]
struct RecordingMcpFakeServer {
    calls: Mutex<Vec<String>>,
}

impl RecordingMcpFakeServer {
    fn call(&self, request: McpToolCallRequest) -> McpToolCallResult {
        let query = request
            .arguments
            .get("query")
            .and_then(McpJsonValue::as_str)
            .unwrap_or_default()
            .to_string();
        self.calls
            .lock()
            .expect("mcp calls should lock")
            .push(format!("{}:{}", request.name, query));
        McpToolCallResult::text(format!("mcp fake server result: {query}"))
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("mcp calls should lock").clone()
    }
}

#[derive(Default)]
struct RecordingPluginToolExecutor {
    requests: Mutex<Vec<PluginToolExecutionRequest>>,
}

impl RecordingPluginToolExecutor {
    fn requests(&self) -> Vec<PluginToolExecutionRequest> {
        self.requests
            .lock()
            .expect("plugin requests should lock")
            .clone()
    }
}

#[async_trait]
impl PluginToolExecutor for RecordingPluginToolExecutor {
    async fn execute(
        &self,
        request: PluginToolExecutionRequest,
    ) -> Result<PluginToolExecutionResult> {
        let topic = request.argument("topic").unwrap_or_default().to_string();
        self.requests
            .lock()
            .expect("plugin requests should lock")
            .push(request);
        Ok(PluginToolExecutionResult::completed(format!(
            "plugin tool result: {topic}"
        )))
    }
}

fn catalog() -> ToolCatalog {
    let mut catalog = ToolCatalog::new();
    catalog.add_tool(
        ToolDescriptor::new("search")
            .with_description("Search docs")
            .with_parameters(json!({
                "type": "object",
                "properties": {
                    "q": {"type": "string"}
                },
                "required": ["q"]
            })),
    );
    catalog.add_tool(ToolDescriptor::new("calc").with_parameters(json!({
        "type": "object",
        "properties": {
            "expr": {"type": "string"}
        }
    })));
    catalog.add_tool(ToolDescriptor::new("draw").with_parameters(json!({
        "type": "object",
        "properties": {
            "prompt": {"type": "string"}
        }
    })));
    catalog
}

fn web_catalog() -> ToolCatalog {
    let mut catalog = builtin_internal_tool_catalog().into_tool_catalog();
    catalog.add_tool(
        ToolDescriptor::new("search")
            .with_description("Search docs")
            .with_parameters(json!({
                "type": "object",
                "properties": {
                    "q": {"type": "string"}
                },
                "required": ["q"]
            })),
    );
    catalog
}

struct StaticWebSearchConfig(WebSearchSessionConfig);

#[async_trait]
impl WebSearchSessionConfigPort for StaticWebSearchConfig {
    async fn web_search_config_for_event(
        &self,
        _event: &MessageEvent,
    ) -> Result<WebSearchSessionConfig> {
        Ok(self.0.clone())
    }
}

struct QueueWebSearchConfig(Mutex<VecDeque<WebSearchSessionConfig>>);

impl QueueWebSearchConfig {
    fn new(configs: Vec<WebSearchSessionConfig>) -> Self {
        Self(Mutex::new(VecDeque::from(configs)))
    }
}

#[async_trait]
impl WebSearchSessionConfigPort for QueueWebSearchConfig {
    async fn web_search_config_for_event(
        &self,
        _event: &MessageEvent,
    ) -> Result<WebSearchSessionConfig> {
        Ok(self
            .0
            .lock()
            .expect("web search config queue should lock")
            .pop_front()
            .unwrap_or_default())
    }
}

fn tool_call(id: &str, name: &str, arguments: Value) -> ProviderToolCall {
    ProviderToolCall::new(id, name, ProviderToolCallArguments::Json(arguments))
}

fn tool_response(calls: Vec<ProviderToolCall>) -> ChatResponse {
    let mut metadata = ProviderResponseMetadata::default();
    for call in calls {
        metadata = metadata.with_tool_call(call);
    }
    ChatResponse::text("").with_metadata(metadata)
}

fn context_contains(request: &ChatRequest, expected: &str) -> bool {
    request
        .contexts
        .iter()
        .any(|context| context_text(context).contains(expected))
}

fn context_has_image_data_url(request: &ChatRequest) -> bool {
    request.contexts.iter().any(|context| {
        context.parts.iter().any(|part| {
            matches!(
                part,
                ProviderContentPart::ImageUrl { url } if url.starts_with("data:image/png;base64,aW1hZ2U=")
            )
        })
    })
}

fn request_text_occurrences(request: &ChatRequest, expected: &str) -> usize {
    request
        .system_prompt
        .as_deref()
        .map(|prompt| prompt.matches(expected).count())
        .unwrap_or_default()
        + request
            .contexts
            .iter()
            .map(|context| context_text(context).matches(expected).count())
            .sum::<usize>()
}

fn context_text(context: &ProviderContextMessage) -> String {
    context
        .parts
        .iter()
        .filter_map(|part| match part {
            ProviderContentPart::Text { text } => Some(text.as_str()),
            ProviderContentPart::ImageUrl { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn mcp_arguments_from_json(
    arguments: &std::collections::BTreeMap<String, Value>,
) -> Result<McpJsonObject> {
    let mut object = McpJsonObject::new();
    for (key, value) in arguments {
        object.0.insert(
            key.clone(),
            McpJsonValue::try_from(value.clone()).map_err(|err| {
                AstrbotError::Pipeline(format!("failed to convert MCP argument {key}: {err}"))
            })?,
        );
    }
    Ok(object)
}

fn mcp_result_text(result: McpToolCallResult) -> String {
    let text = result
        .content
        .into_iter()
        .filter_map(|content| match content {
            McpContentBlock::Text { text } => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() {
        "The MCP tool has no return value.".to_string()
    } else {
        text
    }
}

fn plugin_argument_string(value: Value) -> String {
    match value {
        Value::String(value) => value,
        other => other.to_string(),
    }
}
