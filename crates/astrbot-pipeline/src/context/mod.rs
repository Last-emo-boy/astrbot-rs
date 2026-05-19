mod content_safety;
mod policy;
mod preprocess;
mod provider_preference;
mod quote;
mod result;
mod session;

use std::sync::Arc;

use astrbot_agent::{AgentRunHook, NoopAgentRunHook, NoopProviderRequestHook, ProviderRequestHook};
use astrbot_core::Result;
use astrbot_plugin::PluginRegistry;
use astrbot_provider::{ChatProvider, TextToSpeechProvider};
use astrbot_render::{RenderArtifact, T2iRenderer};
use async_trait::async_trait;

pub use content_safety::{
    BaiduAipContentSafetyStrategy, ContentSafetyConfig, ContentSafetyStrategy,
    ContentSafetyVerdict, KeywordContentSafetyStrategy,
};
pub use policy::{
    ProviderFallbackConfig, RateLimitConfig, RateLimitStrategy, WakeCheckConfig,
    WhitelistPolicyConfig,
};
pub use preprocess::{
    NoopPreAckReactionSink, NoopPreprocessPathMapper, PreAckConfig, PreAckReactionSink,
    PrefixPathMapper, PrefixPathMapping, PreprocessConfig, PreprocessPathMapper,
    SpeechToTextPreprocessConfig, strip_file_scheme,
};
pub use provider_preference::{
    InMemoryProviderPreferencePort, NoProviderPreferencePort, ProviderPreferencePort,
    ScopedProviderPreferencePort,
};
pub use quote::{NoQuoteContextPolicy, QuoteContextPolicy, SelectedTextQuoteContextPolicy};
pub use result::{ResultDecorateConfig, TextToImageDecorateConfig, TextToSpeechDecorateConfig};
pub use session::{
    AllowAllSessionStatusPort, EmptySessionContextPort, SessionContextPort, SessionStatusPort,
};

#[derive(Clone)]
pub struct PipelineContext {
    chat_provider: Option<Arc<dyn ChatProvider>>,
    plugin_registry: Option<Arc<PluginRegistry>>,
    wake_check: WakeCheckConfig,
    whitelist_policy: WhitelistPolicyConfig,
    session_status: Arc<dyn SessionStatusPort>,
    session_context: Arc<dyn SessionContextPort>,
    provider_preference: Arc<dyn ProviderPreferencePort>,
    quote_context: Arc<dyn QuoteContextPolicy>,
    agent_run_hook: Arc<dyn AgentRunHook>,
    provider_request_hook: Arc<dyn ProviderRequestHook>,
    rate_limit: RateLimitConfig,
    content_safety: ContentSafetyConfig,
    preprocess: PreprocessConfig,
    provider_fallback: ProviderFallbackConfig,
    result_decorate: ResultDecorateConfig,
    text_to_speech_provider: Option<Arc<dyn TextToSpeechProvider>>,
    t2i_renderer: Option<Arc<dyn T2iRenderer>>,
    result_file_service: Arc<dyn ResultFileService>,
}

impl Default for PipelineContext {
    fn default() -> Self {
        Self {
            chat_provider: None,
            plugin_registry: None,
            wake_check: WakeCheckConfig::default(),
            whitelist_policy: WhitelistPolicyConfig::default(),
            session_status: Arc::new(AllowAllSessionStatusPort),
            session_context: Arc::new(EmptySessionContextPort),
            provider_preference: Arc::new(NoProviderPreferencePort),
            quote_context: Arc::new(SelectedTextQuoteContextPolicy::default()),
            agent_run_hook: Arc::new(NoopAgentRunHook),
            provider_request_hook: Arc::new(NoopProviderRequestHook),
            rate_limit: RateLimitConfig::default(),
            content_safety: ContentSafetyConfig::default(),
            preprocess: PreprocessConfig::default(),
            provider_fallback: ProviderFallbackConfig::default(),
            result_decorate: ResultDecorateConfig::default(),
            text_to_speech_provider: None,
            t2i_renderer: None,
            result_file_service: Arc::new(NoopResultFileService),
        }
    }
}

impl PipelineContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_chat_provider(provider: Arc<dyn ChatProvider>) -> Self {
        Self {
            chat_provider: Some(provider),
            plugin_registry: None,
            wake_check: WakeCheckConfig::default(),
            whitelist_policy: WhitelistPolicyConfig::default(),
            session_status: Arc::new(AllowAllSessionStatusPort),
            session_context: Arc::new(EmptySessionContextPort),
            provider_preference: Arc::new(NoProviderPreferencePort),
            quote_context: Arc::new(SelectedTextQuoteContextPolicy::default()),
            agent_run_hook: Arc::new(NoopAgentRunHook),
            provider_request_hook: Arc::new(NoopProviderRequestHook),
            rate_limit: RateLimitConfig::default(),
            content_safety: ContentSafetyConfig::default(),
            preprocess: PreprocessConfig::default(),
            provider_fallback: ProviderFallbackConfig::default(),
            result_decorate: ResultDecorateConfig::default(),
            text_to_speech_provider: None,
            t2i_renderer: None,
            result_file_service: Arc::new(NoopResultFileService),
        }
    }

    pub fn with_plugin_registry(mut self, plugin_registry: Arc<PluginRegistry>) -> Self {
        self.plugin_registry = Some(plugin_registry);
        self
    }

    pub fn with_wake_check(mut self, wake_check: WakeCheckConfig) -> Self {
        self.wake_check = wake_check;
        self
    }

    pub fn with_whitelist_policy(mut self, whitelist_policy: WhitelistPolicyConfig) -> Self {
        self.whitelist_policy = whitelist_policy;
        self
    }

    pub fn with_session_status_port(mut self, session_status: Arc<dyn SessionStatusPort>) -> Self {
        self.session_status = session_status;
        self
    }

    pub fn with_session_context_port(
        mut self,
        session_context: Arc<dyn SessionContextPort>,
    ) -> Self {
        self.session_context = session_context;
        self
    }

    pub fn with_provider_preference_port(
        mut self,
        provider_preference: Arc<dyn ProviderPreferencePort>,
    ) -> Self {
        self.provider_preference = provider_preference;
        self
    }

    pub fn with_quote_context_policy(mut self, quote_context: Arc<dyn QuoteContextPolicy>) -> Self {
        self.quote_context = quote_context;
        self
    }

    pub fn with_agent_run_hook(mut self, agent_run_hook: Arc<dyn AgentRunHook>) -> Self {
        self.agent_run_hook = agent_run_hook;
        self
    }

    pub fn with_provider_request_hook(
        mut self,
        provider_request_hook: Arc<dyn ProviderRequestHook>,
    ) -> Self {
        self.provider_request_hook = provider_request_hook;
        self
    }

    pub fn with_rate_limit(mut self, rate_limit: RateLimitConfig) -> Self {
        self.rate_limit = rate_limit;
        self
    }

    pub fn with_content_safety(mut self, content_safety: ContentSafetyConfig) -> Self {
        self.content_safety = content_safety;
        self
    }

    pub fn with_preprocess(mut self, preprocess: PreprocessConfig) -> Self {
        self.preprocess = preprocess;
        self
    }

    pub fn with_provider_fallback(mut self, provider_fallback: ProviderFallbackConfig) -> Self {
        self.provider_fallback = provider_fallback;
        self
    }

    pub fn with_result_decorate(mut self, result_decorate: ResultDecorateConfig) -> Self {
        self.result_decorate = result_decorate;
        self
    }

    pub fn with_text_to_speech_provider(mut self, provider: Arc<dyn TextToSpeechProvider>) -> Self {
        self.text_to_speech_provider = Some(provider);
        self
    }

    pub fn with_t2i_renderer(mut self, renderer: Arc<dyn T2iRenderer>) -> Self {
        self.t2i_renderer = Some(renderer);
        self
    }

    pub fn with_result_file_service(mut self, service: Arc<dyn ResultFileService>) -> Self {
        self.result_file_service = service;
        self
    }

    pub fn chat_provider(&self) -> Option<Arc<dyn ChatProvider>> {
        self.chat_provider.clone()
    }

    pub fn plugin_registry(&self) -> Option<Arc<PluginRegistry>> {
        self.plugin_registry.clone()
    }

    pub fn wake_check(&self) -> &WakeCheckConfig {
        &self.wake_check
    }

    pub fn whitelist_policy(&self) -> &WhitelistPolicyConfig {
        &self.whitelist_policy
    }

    pub fn session_status(&self) -> Arc<dyn SessionStatusPort> {
        self.session_status.clone()
    }

    pub fn session_context(&self) -> Arc<dyn SessionContextPort> {
        self.session_context.clone()
    }

    pub fn provider_preference(&self) -> Arc<dyn ProviderPreferencePort> {
        self.provider_preference.clone()
    }

    pub fn quote_context(&self) -> Arc<dyn QuoteContextPolicy> {
        self.quote_context.clone()
    }

    pub fn agent_run_hook(&self) -> Arc<dyn AgentRunHook> {
        self.agent_run_hook.clone()
    }

    pub fn provider_request_hook(&self) -> Arc<dyn ProviderRequestHook> {
        self.provider_request_hook.clone()
    }

    pub fn rate_limit(&self) -> &RateLimitConfig {
        &self.rate_limit
    }

    pub fn content_safety(&self) -> &ContentSafetyConfig {
        &self.content_safety
    }

    pub fn preprocess(&self) -> &PreprocessConfig {
        &self.preprocess
    }

    pub fn provider_fallback(&self) -> &ProviderFallbackConfig {
        &self.provider_fallback
    }

    pub fn result_decorate(&self) -> &ResultDecorateConfig {
        &self.result_decorate
    }

    pub fn text_to_speech_provider(&self) -> Option<Arc<dyn TextToSpeechProvider>> {
        self.text_to_speech_provider.clone()
    }

    pub fn t2i_renderer(&self) -> Option<Arc<dyn T2iRenderer>> {
        self.t2i_renderer.clone()
    }

    pub fn result_file_service(&self) -> Arc<dyn ResultFileService> {
        self.result_file_service.clone()
    }
}

#[async_trait]
pub trait ResultFileService: Send + Sync {
    async fn public_url(&self, artifact: &RenderArtifact) -> Result<Option<String>>;
}

pub struct NoopResultFileService;

#[async_trait]
impl ResultFileService for NoopResultFileService {
    async fn public_url(&self, _artifact: &RenderArtifact) -> Result<Option<String>> {
        Ok(None)
    }
}
