use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use astrbot_core::{
    MessageChain, MessageEvent, MessageEventResult, ProviderContentPart, ProviderContextMessage,
    ProviderRequest, ProviderToolCallResult, ProviderToolPlaceholder, Result,
};
use astrbot_provider::{
    ChatProvider, ChatRequest, ProviderResponseMetadata, ProviderToolCall,
    ProviderToolCallArguments,
};
use astrbot_tool::{ToolCatalog, ToolDescriptor};
use async_trait::async_trait;
use serde_json::Value;

use crate::{
    AgentDoneEvent, AgentFallbackPolicy, AgentFeedbackEvent, AgentHookEvent, AgentLifecycleEvent,
    AgentRunContext, AgentRunHook, AgentRunOutcome, AgentRunner, AgentStopSignalPolicy,
    AgentStopSignalPort, AgentToolCall, EventStopSignalPort, NoopAgentRunHook,
    NoopProviderRequestDecorator, NoopToolImageCache, ProviderRequestDecorator,
    ProviderRequestEnvelope, ToolImageCachePort, ToolImageCacheRequest,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolLoopStrategy {
    Disabled,
    ProviderToolCalls,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolLoopPolicy {
    pub strategy: ToolLoopStrategy,
    pub max_steps: usize,
    pub tool_call_timeout_seconds: u64,
    pub schema_mode: String,
}

impl Default for ToolLoopPolicy {
    fn default() -> Self {
        Self {
            strategy: ToolLoopStrategy::Disabled,
            max_steps: 30,
            tool_call_timeout_seconds: 60,
            schema_mode: "full".to_string(),
        }
    }
}

impl ToolLoopPolicy {
    pub fn enabled(mut self) -> Self {
        self.strategy = ToolLoopStrategy::ProviderToolCalls;
        self
    }

    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps.max(1);
        self
    }

    pub fn with_timeout_seconds(mut self, tool_call_timeout_seconds: u64) -> Self {
        self.tool_call_timeout_seconds = tool_call_timeout_seconds.max(1);
        self
    }

    pub fn with_schema_mode(mut self, schema_mode: impl Into<String>) -> Self {
        let schema_mode = schema_mode.into();
        if !schema_mode.trim().is_empty() {
            self.schema_mode = schema_mode;
        }
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolLoopState {
    Idle,
    Running,
    Done,
    Error,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolLoopStep {
    pub index: usize,
    pub tool_name: Option<String>,
}

impl ToolLoopStep {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            tool_name: None,
        }
    }

    pub fn with_tool_name(mut self, tool_name: impl Into<String>) -> Self {
        let tool_name = tool_name.into();
        self.tool_name = (!tool_name.trim().is_empty()).then_some(tool_name);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolLoopOutcome {
    pub final_request: ProviderRequest,
    pub steps: Vec<ToolLoopStep>,
    pub state: ToolLoopState,
}

impl ToolLoopOutcome {
    pub fn skipped(final_request: ProviderRequest) -> Self {
        Self {
            final_request,
            steps: Vec::new(),
            state: ToolLoopState::Done,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentToolExecutionRequest {
    pub descriptor: ToolDescriptor,
    pub tool_call_id: String,
    pub session_id: String,
    pub arguments: BTreeMap<String, Value>,
    pub raw_arguments: String,
}

impl AgentToolExecutionRequest {
    pub fn new(
        descriptor: ToolDescriptor,
        tool_call_id: impl Into<String>,
        session_id: impl Into<String>,
        arguments: BTreeMap<String, Value>,
        raw_arguments: impl Into<String>,
    ) -> Self {
        Self {
            descriptor,
            tool_call_id: tool_call_id.into(),
            session_id: session_id.into(),
            arguments,
            raw_arguments: raw_arguments.into(),
        }
    }

    pub fn argument(&self, key: &str) -> Option<&Value> {
        self.arguments.get(key)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentToolOutput {
    Text(String),
    Image {
        base64_data: String,
        mime_type: String,
    },
}

impl AgentToolOutput {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub fn image(base64_data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Image {
            base64_data: base64_data.into(),
            mime_type: mime_type.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentToolExecutionResult {
    pub outputs: Vec<AgentToolOutput>,
    pub direct_to_user: bool,
}

impl AgentToolExecutionResult {
    pub fn completed(text: impl Into<String>) -> Self {
        Self {
            outputs: vec![AgentToolOutput::text(text)],
            direct_to_user: false,
        }
    }

    pub fn with_output(mut self, output: AgentToolOutput) -> Self {
        self.outputs.push(output);
        self
    }

    pub fn direct(text: impl Into<String>) -> Self {
        Self {
            outputs: vec![AgentToolOutput::text(text)],
            direct_to_user: true,
        }
    }

    pub fn image(base64_data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            outputs: vec![AgentToolOutput::image(base64_data, mime_type)],
            direct_to_user: false,
        }
    }

    pub fn into_text(self) -> String {
        self.outputs
            .into_iter()
            .filter_map(|output| match output {
                AgentToolOutput::Text(text) => Some(text),
                AgentToolOutput::Image { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[async_trait]
pub trait AgentToolExecutor: Send + Sync {
    async fn execute(&self, request: AgentToolExecutionRequest)
    -> Result<AgentToolExecutionResult>;
}

#[async_trait]
pub trait AgentToolCatalogFilter: Send + Sync {
    async fn catalog_for_event(
        &self,
        event: &MessageEvent,
        catalog: &ToolCatalog,
    ) -> Result<ToolCatalog>;
}

#[derive(Clone, Debug, Default)]
pub struct NoopAgentToolCatalogFilter;

#[async_trait]
impl AgentToolCatalogFilter for NoopAgentToolCatalogFilter {
    async fn catalog_for_event(
        &self,
        _event: &MessageEvent,
        catalog: &ToolCatalog,
    ) -> Result<ToolCatalog> {
        Ok(catalog.clone())
    }
}

pub struct ToolLoopAgentRunner {
    provider: Arc<dyn ChatProvider>,
    catalog: ToolCatalog,
    tool_executor: Arc<dyn AgentToolExecutor>,
    policy: ToolLoopPolicy,
    fallback_policy: AgentFallbackPolicy,
    request_decorator: Arc<dyn ProviderRequestDecorator>,
    catalog_filter: Arc<dyn AgentToolCatalogFilter>,
    hook: Arc<dyn AgentRunHook>,
    image_cache: Arc<dyn ToolImageCachePort>,
    stop_signal: Arc<dyn AgentStopSignalPort>,
    stop_policy: AgentStopSignalPolicy,
}

impl ToolLoopAgentRunner {
    pub fn new(
        provider: Arc<dyn ChatProvider>,
        catalog: ToolCatalog,
        tool_executor: Arc<dyn AgentToolExecutor>,
    ) -> Self {
        Self {
            provider,
            catalog,
            tool_executor,
            policy: ToolLoopPolicy::default().enabled(),
            fallback_policy: AgentFallbackPolicy::default(),
            request_decorator: Arc::new(NoopProviderRequestDecorator),
            catalog_filter: Arc::new(NoopAgentToolCatalogFilter),
            hook: Arc::new(NoopAgentRunHook),
            image_cache: Arc::new(NoopToolImageCache),
            stop_signal: Arc::new(EventStopSignalPort),
            stop_policy: AgentStopSignalPolicy::default(),
        }
    }

    pub fn with_policy(mut self, policy: ToolLoopPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn with_fallback_policy(mut self, fallback_policy: AgentFallbackPolicy) -> Self {
        self.fallback_policy = fallback_policy;
        self
    }

    pub fn with_request_decorator(
        mut self,
        request_decorator: Arc<dyn ProviderRequestDecorator>,
    ) -> Self {
        self.request_decorator = request_decorator;
        self
    }

    pub fn with_catalog_filter(mut self, catalog_filter: Arc<dyn AgentToolCatalogFilter>) -> Self {
        self.catalog_filter = catalog_filter;
        self
    }

    pub fn with_hook(mut self, hook: Arc<dyn AgentRunHook>) -> Self {
        self.hook = hook;
        self
    }

    pub fn with_image_cache(mut self, image_cache: Arc<dyn ToolImageCachePort>) -> Self {
        self.image_cache = image_cache;
        self
    }

    pub fn with_stop_signal(mut self, stop_signal: Arc<dyn AgentStopSignalPort>) -> Self {
        self.stop_signal = stop_signal;
        self
    }

    pub fn with_stop_policy(mut self, stop_policy: AgentStopSignalPolicy) -> Self {
        self.stop_policy = stop_policy;
        self
    }

    async fn call_provider(
        &self,
        request: &ProviderRequest,
    ) -> Result<astrbot_provider::ChatResponse> {
        self.provider.chat(ChatRequest::from(request.clone())).await
    }

    async fn should_stop(&self, event: &MessageEvent) -> Result<bool> {
        let signal = self.stop_signal.stop_signal(event).await?;
        Ok(self.stop_policy.evaluate(&signal).is_some())
    }

    fn attach_tool_placeholders(&self, request: &mut ProviderRequest, catalog: &ToolCatalog) {
        request.tool_placeholders.clear();
        for tool in catalog.tools().iter().filter(|tool| tool.active) {
            let mut placeholder = ProviderToolPlaceholder::new(tool.name.clone());
            if let Some(description) = &tool.description {
                placeholder = placeholder.with_description(description.clone());
            }
            request.tool_placeholders.push(placeholder);
        }
    }

    async fn maybe_requery_tool_arguments(
        &self,
        request: &ProviderRequest,
        response_metadata: &ProviderResponseMetadata,
    ) -> Result<Option<ProviderResponseMetadata>> {
        if !is_skills_like_mode(&self.policy.schema_mode) {
            return Ok(None);
        }

        let needs_requery = response_metadata
            .tool_calls
            .iter()
            .any(|call| tool_arguments_map(call).is_empty());
        if !needs_requery || response_metadata.tool_calls.is_empty() {
            return Ok(None);
        }

        let tool_names = response_metadata
            .tool_calls
            .iter()
            .map(|call| call.name.as_str())
            .collect::<BTreeSet<_>>();
        let instruction = format!(
            "You have decided to call tool(s): {}. Now call the tool(s) with required arguments using the tool schema, and follow the existing tool-use rules.",
            tool_names.iter().copied().collect::<Vec<_>>().join(", ")
        );

        let mut requery_request = request.clone();
        requery_request.system_prompt = Some(match request.system_prompt.as_deref() {
            Some(system_prompt) if !system_prompt.trim().is_empty() => {
                format!("{system_prompt}\n{instruction}")
            }
            _ => instruction,
        });
        requery_request
            .tool_placeholders
            .retain(|tool| tool_names.contains(tool.name.as_str()));

        let response = self.call_provider(&requery_request).await?;
        if response.metadata.tool_calls.is_empty() {
            Ok(None)
        } else {
            Ok(Some(response.metadata))
        }
    }

    async fn execute_tool_call(
        &self,
        lifecycle: &AgentLifecycleEvent,
        catalog: &ToolCatalog,
        request: &mut ProviderRequest,
        call: &ProviderToolCall,
        feedback: &mut Vec<AgentFeedbackEvent>,
    ) -> Result<bool> {
        let agent_call = agent_tool_call_from_provider(call);
        feedback.push(AgentFeedbackEvent::tool_call(format!(
            "Calling tool: {}",
            agent_call.name
        )));

        let Some(descriptor) = catalog.tool(&call.name).filter(|tool| tool.active).cloned() else {
            let result = ProviderToolCallResult::new(
                call.id.clone(),
                call.name.clone(),
                format!("error: Tool {} not found.", call.name),
            );
            request.tool_call_results.push(result.clone());
            request.contexts.push(tool_result_context(&result));
            feedback.push(AgentFeedbackEvent::tool_result(result.content.clone()));
            return Ok(false);
        };

        let arguments = filtered_tool_arguments(&descriptor, call);
        let execution_request = AgentToolExecutionRequest::new(
            descriptor,
            call.id.clone(),
            request.session_id.clone().unwrap_or_default(),
            arguments,
            call.arguments_json(),
        );

        self.hook
            .on_event(AgentHookEvent::ToolStart(
                crate::AgentToolLifecycleEvent::start(lifecycle.clone(), agent_call.clone()),
            ))
            .await?;

        let execution = self.tool_executor.execute(execution_request).await;
        let (result, should_stop_loop) = match execution {
            Ok(result) => {
                self.tool_result_from_execution(request, call, result)
                    .await?
            }
            Err(err) => (
                ProviderToolCallResult::new(
                    call.id.clone(),
                    call.name.clone(),
                    format!("error: {err}"),
                ),
                false,
            ),
        };

        request.tool_call_results.push(result.clone());
        if should_append_tool_result_context(request, call, &result) {
            request.contexts.push(tool_result_context(&result));
        }
        feedback.push(AgentFeedbackEvent::tool_result(result.content.clone()));

        self.hook
            .on_event(AgentHookEvent::ToolEnd(
                crate::AgentToolLifecycleEvent::end(lifecycle.clone(), agent_call, result),
            ))
            .await?;

        Ok(should_stop_loop)
    }

    async fn tool_result_from_execution(
        &self,
        request: &mut ProviderRequest,
        call: &ProviderToolCall,
        execution: AgentToolExecutionResult,
    ) -> Result<(ProviderToolCallResult, bool)> {
        let mut text_parts = Vec::new();
        let direct_to_user = execution.direct_to_user;

        for (index, output) in execution.outputs.into_iter().enumerate() {
            match output {
                AgentToolOutput::Text(text) => {
                    if !text.trim().is_empty() {
                        text_parts.push(text);
                    }
                }
                AgentToolOutput::Image {
                    base64_data,
                    mime_type,
                } => {
                    let cached = self
                        .image_cache
                        .save_image(
                            ToolImageCacheRequest::new(
                                base64_data,
                                call.id.clone(),
                                call.name.clone(),
                            )
                            .with_index(index)
                            .with_mime_type(mime_type),
                        )
                        .await?;
                    text_parts.push(format!(
                        "Image returned and cached at uri='{}'. Review the image below. Use send_message_to_user to send it to the user if satisfied.",
                        cached.uri
                    ));
                    if !cached.uri.trim().is_empty()
                        && let Some(image) = self
                            .image_cache
                            .get_image(&cached.uri, &cached.mime_type)
                            .await?
                    {
                        request.contexts.push(ProviderContextMessage::new(
                            "user",
                            vec![
                                ProviderContentPart::text(format!(
                                    "[Image from tool '{}', uri='{}']",
                                    cached.tool_name, cached.uri
                                )),
                                ProviderContentPart::image_url(format!(
                                    "data:{};base64,{}",
                                    image.mime_type, image.base64_data
                                )),
                            ],
                        ));
                    }
                }
            }
        }

        let content = if text_parts.is_empty() {
            "The tool has no return value, or has sent the result directly to the user.".to_string()
        } else {
            text_parts.join("\n")
        };

        Ok((
            ProviderToolCallResult::new(call.id.clone(), call.name.clone(), content),
            direct_to_user,
        ))
    }
}

#[async_trait]
impl AgentRunner for ToolLoopAgentRunner {
    async fn run(&self, event: &MessageEvent) -> Result<AgentRunOutcome> {
        if event.result().is_some() || event.is_stopped() {
            return Ok(AgentRunOutcome::continue_without_result());
        }

        if !self.fallback_policy.enabled {
            return Ok(AgentRunOutcome::continue_without_result());
        }

        let Some(mut envelope) = ProviderRequestEnvelope::from_event(event) else {
            return Ok(AgentRunOutcome::continue_without_result());
        };

        if !envelope.explicit && self.fallback_policy.require_wake && !event.is_at_or_wake_command()
        {
            return Ok(AgentRunOutcome::continue_without_result());
        }

        let run_context = AgentRunContext::from_event(event);
        let lifecycle = AgentLifecycleEvent::from_context(&run_context);
        self.hook
            .on_event(AgentHookEvent::AgentBegin(lifecycle.clone()))
            .await?;

        self.request_decorator
            .decorate(event, &mut envelope.request)
            .await?;
        let catalog = self
            .catalog_filter
            .catalog_for_event(event, &self.catalog)
            .await?;
        if self.policy.strategy == ToolLoopStrategy::ProviderToolCalls {
            self.attach_tool_placeholders(&mut envelope.request, &catalog);
        }

        let mut feedback = Vec::new();
        let mut step_count = 0;
        loop {
            if self.should_stop(event).await? {
                return Ok(AgentRunOutcome::continue_without_result()
                    .with_feedback_events(feedback)
                    .with_feedback_event(AgentFeedbackEvent::new(
                        crate::AgentFeedbackEventKind::Aborted,
                        MessageChain::default(),
                    )));
            }

            let response = match self.call_provider(&envelope.request).await {
                Ok(response) => response,
                Err(err) => {
                    let Some(message) = self.fallback_policy.error_message.clone() else {
                        return Err(err);
                    };
                    return Ok(
                        AgentRunOutcome::with_result(MessageEventResult::general(message))
                            .with_feedback_events(feedback),
                    );
                }
            };

            let mut metadata = response.metadata.clone();
            if let Some(requeried) = self
                .maybe_requery_tool_arguments(&envelope.request, &metadata)
                .await?
            {
                metadata = requeried;
            }

            if self.policy.strategy == ToolLoopStrategy::Disabled || metadata.tool_calls.is_empty()
            {
                let mut done_event = AgentDoneEvent::new(lifecycle, response.chain.clone());
                if let Some(reasoning) = metadata
                    .reasoning
                    .as_ref()
                    .filter(|reasoning| !reasoning.content.trim().is_empty())
                {
                    done_event = done_event.with_reasoning_content(reasoning.content.clone());
                }
                self.hook
                    .on_event(AgentHookEvent::AgentDone(done_event))
                    .await?;
                return Ok(
                    AgentRunOutcome::with_result(MessageEventResult::llm(response.chain))
                        .with_feedback_events(feedback),
                );
            }

            step_count += 1;
            envelope.request.contexts.push(assistant_tool_call_context(
                &response.chain,
                &metadata.tool_calls,
            ));

            let mut direct_result = None;
            for call in &metadata.tool_calls {
                if self
                    .execute_tool_call(
                        &lifecycle,
                        &catalog,
                        &mut envelope.request,
                        call,
                        &mut feedback,
                    )
                    .await?
                {
                    direct_result = envelope
                        .request
                        .tool_call_results
                        .last()
                        .map(|result| result.content.clone());
                    break;
                }
            }

            envelope.request.prompt = None;
            envelope.request.image_urls.clear();
            envelope.request.extra_user_content_parts.clear();

            if let Some(content) = direct_result {
                return Ok(
                    AgentRunOutcome::with_result(MessageEventResult::general(content))
                        .with_feedback_events(feedback),
                );
            }

            if step_count >= self.policy.max_steps {
                envelope.request.tool_placeholders.clear();
                envelope.request.contexts.push(ProviderContextMessage::text(
                    "user",
                    "工具调用次数已达到上限，请停止使用工具，并根据已经收集到的信息，对你的任务和发现进行总结，然后直接回复用户。",
                ));
                let response = self.call_provider(&envelope.request).await?;
                let done_event = AgentDoneEvent::new(lifecycle, response.chain.clone());
                self.hook
                    .on_event(AgentHookEvent::AgentDone(done_event))
                    .await?;
                return Ok(
                    AgentRunOutcome::with_result(MessageEventResult::llm(response.chain))
                        .with_feedback_events(feedback),
                );
            }
        }
    }
}

fn is_skills_like_mode(schema_mode: &str) -> bool {
    matches!(schema_mode.trim(), "skills_like" | "skills-like")
}

fn agent_tool_call_from_provider(call: &ProviderToolCall) -> AgentToolCall {
    AgentToolCall::new(call.id.clone(), call.name.clone()).with_arguments(call.arguments_json())
}

fn assistant_tool_call_context(
    chain: &MessageChain,
    calls: &[ProviderToolCall],
) -> ProviderContextMessage {
    let content = calls
        .iter()
        .map(|call| {
            format!(
                "tool_call id={} name={} arguments={}",
                call.id,
                call.name,
                call.arguments_json()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let text = if chain.plain_text().trim().is_empty() {
        content
    } else {
        format!("{}\n{}", chain.plain_text(), content)
    };
    ProviderContextMessage::text("assistant", text)
}

fn tool_result_context(result: &ProviderToolCallResult) -> ProviderContextMessage {
    ProviderContextMessage::text("tool", result.content.clone())
}

fn should_append_tool_result_context(
    request: &ProviderRequest,
    call: &ProviderToolCall,
    result: &ProviderToolCallResult,
) -> bool {
    if call.name != "astr_kb_search" {
        return true;
    }
    let content = result.content.trim();
    if content.is_empty() {
        return true;
    }
    request
        .system_prompt
        .as_deref()
        .map(|prompt| !prompt.contains(content))
        .unwrap_or(true)
}

fn filtered_tool_arguments(
    descriptor: &ToolDescriptor,
    call: &ProviderToolCall,
) -> BTreeMap<String, Value> {
    let mut args = tool_arguments_map(call);
    let expected = descriptor
        .parameters
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| properties.keys().cloned().collect::<BTreeSet<_>>());

    if let Some(expected) = expected {
        args.retain(|key, _| expected.contains(key));
    }
    args
}

fn tool_arguments_map(call: &ProviderToolCall) -> BTreeMap<String, Value> {
    match &call.arguments {
        ProviderToolCallArguments::Json(Value::Object(map)) => map
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        ProviderToolCallArguments::PartialJson(raw) => serde_json::from_str::<Value>(raw)
            .ok()
            .and_then(|value| match value {
                Value::Object(map) => Some(map.into_iter().collect::<BTreeMap<String, Value>>()),
                _ => None,
            })
            .unwrap_or_default(),
        _ => BTreeMap::new(),
    }
}
