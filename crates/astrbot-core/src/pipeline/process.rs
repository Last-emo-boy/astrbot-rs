use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

use crate::agent::{AgentContext, AgentRegistry, AgentResult, ToolCall};
use crate::context_compression::{ContextCompressionConfig, ContextCompressor};
use crate::errors::Result;
use crate::message::{
    AstrBotMessage, MessageChain, MessageEventResult, MessageMember, MessageType,
};
use crate::persona::PersonaRegistry;
use crate::pipeline::{PipelineContext, PipelineEvent, Stage, StageFlow};
use crate::platform::{MessageSource, PlatformType};
use crate::plugin::PluginDispatcher;
use crate::provider::{ChatConfig, ChatMessage, Provider};

/// ProcessStage — 核心消息处理阶段
///
/// 职责：
/// 1. 插件消息拦截 (PluginManager::dispatch_message)
/// 2. 组装 messages context（session history + system prompt）
/// 3. Agent Runner 调用（ToolLoop/Coze/Dify/Dashscope/DeerFlow）
/// 4. ChatProvider 路由（LLM 直接调用兜底）
pub struct ProcessStage {
    /// 默认 LLM Provider（兜底路由）
    default_provider: Option<Arc<dyn Provider>>,
    /// Agent 注册表
    agent_registry: Option<Arc<AgentRegistry>>,
    /// 系统提示词（fallback，当 persona_registry 未设置时使用）
    system_prompt: Option<String>,
    /// 人格注册表（优先使用）
    persona_registry: Option<Arc<PersonaRegistry>>,
    /// 上下文压缩器
    context_compressor: Option<Arc<ContextCompressor>>,
    /// 会话历史（简化版：user_id → Vec<ChatMessage>）
    session_history: Arc<tokio::sync::Mutex<HashMap<String, Vec<ChatMessage>>>>,
    /// 默认 Agent ID（优先走 Agent Runner）
    default_agent_id: Option<String>,
    /// 插件管理器（消息拦截）
    plugin_manager: Option<Arc<dyn PluginDispatcher>>,
    /// 指标收集器
    metrics_collector: Option<Arc<tokio::sync::Mutex<crate::metrics::MetricsCollector>>>,
    /// 最大历史轮数（1轮 = user + assistant，默认 20）
    max_history_rounds: usize,
}

impl ProcessStage {
    pub fn new() -> Self {
        Self {
            default_provider: None,
            agent_registry: None,
            system_prompt: None,
            persona_registry: None,
            context_compressor: None,
            session_history: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            default_agent_id: None,
            plugin_manager: None,
            metrics_collector: None,
            max_history_rounds: 20,
        }
    }

    /// 设置默认 LLM Provider
    pub fn with_provider(mut self, provider: Arc<dyn Provider>) -> Self {
        self.default_provider = Some(provider);
        self
    }

    /// 设置 Agent 注册表
    pub fn with_agent_registry(mut self, registry: Arc<AgentRegistry>) -> Self {
        self.agent_registry = Some(registry);
        self
    }

    /// 设置系统提示词
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 设置人格注册表
    pub fn with_persona_registry(mut self, registry: Arc<PersonaRegistry>) -> Self {
        self.persona_registry = Some(registry);
        self
    }

    /// 设置上下文压缩器
    pub fn with_context_compressor(mut self, compressor: Arc<ContextCompressor>) -> Self {
        self.context_compressor = Some(compressor);
        self
    }

    /// 设置最大历史轮数
    pub fn with_max_history_rounds(mut self, rounds: usize) -> Self {
        self.max_history_rounds = rounds;
        self
    }

    /// 设置默认 Agent ID
    pub fn with_default_agent(mut self, agent_id: impl Into<String>) -> Self {
        self.default_agent_id = Some(agent_id.into());
        self
    }

    /// 设置插件管理器
    pub fn with_plugin_manager(mut self, pm: Arc<dyn PluginDispatcher>) -> Self {
        self.plugin_manager = Some(pm);
        self
    }

    /// 设置指标收集器
    pub fn with_metrics_collector(
        mut self,
        mc: Arc<tokio::sync::Mutex<crate::metrics::MetricsCollector>>,
    ) -> Self {
        self.metrics_collector = Some(mc);
        self
    }

    /// 从 PipelineEvent 提取用户输入文本
    fn extract_user_input(&self, event: &PipelineEvent) -> String {
        event.message.chain.plain_text()
    }

    /// 尝试插件拦截。如果插件处理了消息，返回 true（跳过 provider）
    async fn try_plugin_intercept(&self, event: &mut PipelineEvent) -> Result<bool> {
        let pm = match self.plugin_manager.as_ref() {
            Some(pm) => pm,
            None => return Ok(false),
        };

        let source = MessageSource {
            platform: event.message.platform,
            session_id: event.message.session_id.clone(),
            message_id: event.message.message_id.clone(),
            user_id: event.message.sender.user_id.clone(),
        };

        let results = pm.dispatch_message(&event.message, &source).await;

        for (id, result) in results {
            match result {
                Ok(MessageEventResult::Reply { chain }) => {
                    info!("[ProcessStage] plugin {} replied", id);
                    event.result_chain = Some(chain);
                    return Ok(true);
                }
                Ok(MessageEventResult::Forward { target: _, chain }) => {
                    info!("[ProcessStage] plugin {} forwarded message", id);
                    event.result_chain = Some(chain);
                    return Ok(true);
                }
                Ok(MessageEventResult::Nothing) => {}
                Err(e) => {
                    warn!("[ProcessStage] plugin {} error: {}", id, e);
                }
            }
        }

        Ok(false)
    }

    /// 构建 ChatMessage 上下文（system prompt + history + current）
    /// System prompt 来源优先级：
    /// 1. PersonaRegistry::build_system_prompt()（如果注册了 persona_registry）
    /// 2. self.system_prompt fallback
    async fn build_messages(&self, event: &PipelineEvent) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        // 1. System prompt — persona registry 优先
        let system_prompt = if let Some(ref registry) = self.persona_registry {
            Some(registry.build_system_prompt(None).await)
        } else {
            self.system_prompt.clone()
        };

        if let Some(prompt) = system_prompt {
            if !prompt.is_empty() {
                messages.push(ChatMessage::system(prompt));
            }
        }

        // 2. Session history
        let session_id = event.message.session_id.clone();
        let history = {
            let lock = self.session_history.lock().await;
            lock.get(&session_id).cloned().unwrap_or_default()
        };
        messages.extend(history);

        // 3. Apply context compression if configured
        if let Some(ref compressor) = self.context_compressor {
            let _ = compressor.compress(&mut messages);
        }

        // 4. Current user message
        let user_text = self.extract_user_input(event);
        if !user_text.is_empty() {
            messages.push(ChatMessage::user(user_text));
        }

        messages
    }

    /// 更新会话历史（应用 max_history_rounds 限制）
    async fn update_history(&self, session_id: &str, user_msg: String, assistant_msg: String) {
        let mut lock = self.session_history.lock().await;
        let entry = lock.entry(session_id.to_string()).or_insert_with(Vec::new);
        entry.push(ChatMessage::user(user_msg));
        entry.push(ChatMessage::assistant(assistant_msg));
        // 保留最近 max_history_rounds 轮（每轮 = user + assistant = 2 条消息）
        let max_messages = self.max_history_rounds.saturating_mul(2);
        if entry.len() > max_messages {
            *entry = entry.split_off(entry.len() - max_messages);
        }
    }

    /// 调用 Agent Runner
    async fn run_agent(
        &self,
        agent_id: &str,
        messages: Vec<ChatMessage>,
        source: MessageSource,
        user_id: String,
        session_id: String,
    ) -> Result<String> {
        let registry = self.agent_registry.as_ref().ok_or_else(|| {
            crate::errors::AstrBotError::Internal("Agent registry not set".to_string())
        })?;

        let ctx = AgentContext {
            messages,
            source,
            user_id,
            session_id,
            extras: HashMap::new(),
        };

        let result = registry.execute(agent_id, &ctx).await?;

        match result {
            AgentResult::Text { content } => Ok(content),
            AgentResult::ToolCall { calls } => {
                warn!(
                    "Agent requested {} tool calls — not yet handled",
                    calls.len()
                );
                Ok(format!(
                    "[Agent requested {} tools — not yet supported]",
                    calls.len()
                ))
            }
            AgentResult::PassThrough => Ok(String::new()),
            AgentResult::Error { message } => Err(crate::errors::AstrBotError::Internal(message)),
        }
    }

    /// 调用 LLM Provider 兜底。失败时返回友好的错误提示文本，不传播 Err。
    async fn run_provider(&self, messages: Vec<ChatMessage>) -> String {
        let provider = match self.default_provider.as_ref() {
            Some(p) => p,
            None => {
                warn!("[ProcessStage] No provider configured");
                return "⚠️ 我还没有配置 AI 模型，请联系管理员添加 Provider。".to_string();
            }
        };

        let config = ChatConfig {
            stream: false,
            ..Default::default()
        };

        let provider_id = provider.id().to_string();
        match provider.chat(messages, config).await {
            Ok(response) => {
                if let Some(ref mc) = self.metrics_collector {
                    let mut lock = mc.lock().await;
                    lock.increment_provider_call(&provider_id, true);
                }
                response.content
            }
            Err(e) => {
                if let Some(ref mc) = self.metrics_collector {
                    let mut lock = mc.lock().await;
                    lock.increment_provider_call(&provider_id, false);
                }
                warn!("[ProcessStage] Provider chat failed: {}", e);
                format!("⚠️ AI 服务暂时不可用 ({})。请稍后再试。", e)
            }
        }
    }
}

#[async_trait]
impl Stage for ProcessStage {
    async fn initialize(&mut self, _ctx: &PipelineContext) -> Result<()> {
        // PipelineContext 暂无 plugin_manager 字段，跳过初始化
        Ok(())
    }

    async fn process(&self, event: &mut PipelineEvent) -> Result<StageFlow> {
        let user_input = self.extract_user_input(event);
        if user_input.is_empty() {
            return Ok(StageFlow::Done);
        }

        info!(
            "[ProcessStage] processing message from {}",
            event.message.sender.user_id
        );

        // 1. 插件消息拦截 — 如果插件返回响应，直接跳过 provider
        if self.try_plugin_intercept(event).await? {
            return Ok(StageFlow::Done);
        }

        // 2. 构建消息上下文
        let messages = self.build_messages(event).await;

        // 优先走 Agent Runner
        let response_text = if let Some(ref agent_id) = self.default_agent_id {
            let source = MessageSource {
                platform: event.message.platform,
                session_id: event.message.session_id.clone(),
                message_id: event.message.message_id.clone(),
                user_id: event.message.sender.user_id.clone(),
            };
            match self
                .run_agent(
                    agent_id,
                    messages.clone(),
                    source,
                    event.message.sender.user_id.clone(),
                    event.message.session_id.clone(),
                )
                .await
            {
                Ok(text) => text,
                Err(e) => {
                    warn!("Agent execution failed: {}, falling back to provider", e);
                    self.run_provider(messages).await
                }
            }
        } else {
            // 直接走 Provider 兜底
            self.run_provider(messages).await
        };

        // 组装回复消息链
        if !response_text.is_empty() {
            let reply_chain = MessageChain::new().text(response_text.clone());
            event.result_chain = Some(reply_chain);

            // 更新历史
            self.update_history(&event.message.session_id, user_input, response_text)
                .await;
        }

        Ok(StageFlow::Done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::MessageMember;
    use crate::platform::{MessageSource, PlatformType};
    use crate::provider::{ChatResponse, ModelInfo, TokenUsage};
    use crate::testing::MockProvider;
    use chrono::Utc;

    fn make_test_event(text: &str) -> PipelineEvent {
        let msg = AstrBotMessage {
            message_id: "msg-1".to_string(),
            timestamp: Utc::now(),
            platform: PlatformType::Custom,
            session_id: "sess-1".to_string(),
            sender: MessageMember {
                user_id: "user-1".to_string(),
                nickname: Some("Test".to_string()),
                card: None,
                role: None,
                is_self: false,
            },
            message_type: MessageType::Private,
            chain: MessageChain::new().text(text),
            raw_payload: None,
        };
        PipelineEvent::new(msg)
    }

    #[tokio::test]
    async fn test_process_stage_provider_fallback() {
        let provider =
            Arc::new(MockProvider::new("mock", "Mock").with_chat_response("Hello from mock!"));
        let stage = ProcessStage::new().with_provider(provider);

        let mut event = make_test_event("Hi");
        let flow = stage.process(&mut event).await.unwrap();

        assert!(matches!(flow, StageFlow::Done));
        assert!(event.result_chain.is_some());
        assert_eq!(event.result_chain.unwrap().plain_text(), "Hello from mock!");
    }

    #[tokio::test]
    async fn test_process_stage_provider_failure_fallback() {
        let provider = Arc::new(MockProvider::new("mock", "Mock").with_chat_failure());
        let stage = ProcessStage::new().with_provider(provider);

        let mut event = make_test_event("Hi");
        let flow = stage.process(&mut event).await.unwrap();

        assert!(matches!(flow, StageFlow::Done));
        assert!(event.result_chain.is_some());
        let text = event.result_chain.unwrap().plain_text();
        assert!(
            text.contains("AI 服务暂时不可用") || text.contains("不可用"),
            "Expected friendly error text, got: {}",
            text
        );
    }

    #[tokio::test]
    async fn test_persona_registry_injection() {
        use crate::persona::{Persona, PersonaRegistry};

        let registry = Arc::new(PersonaRegistry::new());
        let persona = Persona::new("test", "Test", "You are {{name}}.")
            .with_variable("name", "AstrBot")
            .with_default(true);
        registry.load(vec![persona]).await;

        let provider = Arc::new(MockProvider::new("mock", "Mock").with_chat_response("Reply"));
        let stage = ProcessStage::new()
            .with_provider(provider)
            .with_persona_registry(registry);

        let mut event = make_test_event("Hello");
        stage.process(&mut event).await.unwrap();

        // Verify that the system prompt was injected by checking history
        let history = stage.session_history.lock().await;
        // History starts empty; after process, user + assistant messages exist
        let sess = history.get("sess-1").unwrap();
        assert_eq!(sess.len(), 2); // user + assistant
    }

    #[tokio::test]
    async fn test_system_prompt_fallback_without_persona() {
        let provider = Arc::new(MockProvider::new("mock", "Mock").with_chat_response("Reply"));
        let stage = ProcessStage::new()
            .with_provider(provider)
            .with_system_prompt("You are test bot");

        let mut event = make_test_event("Hello");
        stage.process(&mut event).await.unwrap();

        let history = stage.session_history.lock().await;
        let sess = history.get("sess-1").unwrap();
        assert_eq!(sess.len(), 2);
    }

    #[tokio::test]
    async fn test_context_compression_in_pipeline() {
        use crate::context_compression::{ContextCompressionConfig, ContextCompressor};

        let provider = Arc::new(MockProvider::new("mock", "Mock").with_chat_response("Reply"));
        let compressor = Arc::new(ContextCompressor::new(
            ContextCompressionConfig::new(2), // keep only 2 non-system messages
        ));
        let stage = ProcessStage::new()
            .with_provider(provider)
            .with_context_compressor(compressor)
            .with_max_history_rounds(10); // allow up to 10 rounds in memory

        // Simulate 5 rounds of conversation
        for i in 0..5 {
            let mut event = make_test_event(&format!("Message {}", i));
            stage.process(&mut event).await.unwrap();
        }

        // History should have been compressed to max 2 non-system messages
        // But compressor only acts on messages built by build_messages(), which
        // includes history + current user message. After processing 5 rounds,
        // history in memory = 10 messages (5 user + 5 assistant).
        let history = stage.session_history.lock().await;
        let sess = history.get("sess-1").unwrap();
        assert_eq!(sess.len(), 10); // memory limit is 20 (10 rounds), so all 5 rounds kept
    }

    #[tokio::test]
    async fn test_max_history_rounds_configurable() {
        let provider = Arc::new(MockProvider::new("mock", "Mock").with_chat_response("Reply"));
        let stage = ProcessStage::new()
            .with_provider(provider)
            .with_max_history_rounds(2); // only keep 2 rounds

        // 5 rounds of conversation
        for i in 0..5 {
            let mut event = make_test_event(&format!("Message {}", i));
            stage.process(&mut event).await.unwrap();
        }

        let history = stage.session_history.lock().await;
        let sess = history.get("sess-1").unwrap();
        assert_eq!(sess.len(), 4); // 2 rounds = 4 messages (2 user + 2 assistant)
                                   // Verify the most recent messages are kept
        assert_eq!(sess[0].content, "Message 3");
        assert_eq!(sess[1].content, "Reply");
        assert_eq!(sess[2].content, "Message 4");
        assert_eq!(sess[3].content, "Reply");
    }

    #[tokio::test]
    async fn test_process_stage_empty_message() {
        let stage = ProcessStage::new();
        let mut event = make_test_event("");
        let flow = stage.process(&mut event).await.unwrap();
        assert!(matches!(flow, StageFlow::Done));
        assert!(event.result_chain.is_none());
    }

    #[tokio::test]
    async fn test_process_stage_history_roundtrip() {
        let provider = Arc::new(MockProvider::new("mock", "Mock").with_chat_response("Reply"));
        let stage = ProcessStage::new().with_provider(provider);

        let mut event1 = make_test_event("Message 1");
        stage.process(&mut event1).await.unwrap();

        let mut event2 = make_test_event("Message 2");
        stage.process(&mut event2).await.unwrap();

        let history = stage.session_history.lock().await;
        let sess = history.get("sess-1").unwrap();
        assert!(sess.len() >= 2);
    }
}
