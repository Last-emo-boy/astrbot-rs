use std::sync::Arc;

use astrbot_agent::{
    AgentFallbackPolicy, AgentProviderPreferencePort, AgentQuoteContextPort, AgentRunner,
    AgentSessionContextPort, ChatAgentRunner, CompositeProviderRequestDecorator,
    ProviderPreferenceRequestDecorator, ProviderRequestDecorator, QuoteContextRequestDecorator,
    SessionContextRequestDecorator,
};
use astrbot_core::{MessageEvent, ProviderContentPart, ProviderContextMessage, Result};
use async_trait::async_trait;

use crate::{
    PipelineContext, PipelineControl, PipelineStage, ProviderFallbackConfig,
    ProviderPreferencePort, QuoteContextPolicy, SessionContextPort,
};

#[derive(Default)]
pub struct ProviderStage;

#[async_trait]
impl PipelineStage for ProviderStage {
    fn name(&self) -> &str {
        "provider"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        run_provider_fallback(event, ctx).await
    }
}

pub(super) async fn run_provider_fallback(
    event: &mut MessageEvent,
    ctx: &PipelineContext,
) -> Result<PipelineControl> {
    let Some(provider) = ctx.chat_provider() else {
        return Ok(PipelineControl::Continue);
    };

    let runner = ChatAgentRunner::new(provider)
        .with_fallback_policy(agent_fallback_policy(ctx.provider_fallback()))
        .with_request_decorator(agent_request_decorator(ctx))
        .with_request_hook(ctx.provider_request_hook())
        .with_hook(ctx.agent_run_hook());

    if let Some(result) = runner.run(event).await?.into_result() {
        event.set_result(result);
    }
    Ok(PipelineControl::Continue)
}

fn agent_fallback_policy(config: &ProviderFallbackConfig) -> AgentFallbackPolicy {
    let mut policy = if config.enabled {
        AgentFallbackPolicy::default()
    } else {
        AgentFallbackPolicy::disabled()
    };
    policy.require_wake = config.require_wake;
    policy.error_message = config.error_message.clone();
    policy.provider_wake_prefix = config.provider_wake_prefix.clone();
    policy
}

fn agent_request_decorator(ctx: &PipelineContext) -> Arc<dyn ProviderRequestDecorator> {
    Arc::new(
        CompositeProviderRequestDecorator::new()
            .with_decorator(Arc::new(ProviderPreferenceRequestDecorator::new(Arc::new(
                PipelineProviderPreferencePort::new(ctx.provider_preference()),
            ))))
            .with_decorator(Arc::new(SessionContextRequestDecorator::new(Arc::new(
                PipelineSessionContextPort::new(ctx.session_context()),
            ))))
            .with_decorator(Arc::new(QuoteContextRequestDecorator::new(Arc::new(
                PipelineQuoteContextPort::new(ctx.quote_context()),
            )))),
    )
}

struct PipelineProviderPreferencePort {
    inner: Arc<dyn ProviderPreferencePort>,
}

impl PipelineProviderPreferencePort {
    fn new(inner: Arc<dyn ProviderPreferencePort>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl AgentProviderPreferencePort for PipelineProviderPreferencePort {
    async fn preferred_chat_provider_id(&self, event: &MessageEvent) -> Result<Option<String>> {
        self.inner.preferred_chat_provider_id(event).await
    }
}

struct PipelineSessionContextPort {
    inner: Arc<dyn SessionContextPort>,
}

impl PipelineSessionContextPort {
    fn new(inner: Arc<dyn SessionContextPort>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl AgentSessionContextPort for PipelineSessionContextPort {
    async fn context_messages(&self, event: &MessageEvent) -> Result<Vec<ProviderContextMessage>> {
        self.inner.context_messages(event).await
    }
}

struct PipelineQuoteContextPort {
    inner: Arc<dyn QuoteContextPolicy>,
}

impl PipelineQuoteContextPort {
    fn new(inner: Arc<dyn QuoteContextPolicy>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl AgentQuoteContextPort for PipelineQuoteContextPort {
    async fn quote_content_parts(&self, event: &MessageEvent) -> Result<Vec<ProviderContentPart>> {
        self.inner.quote_content_parts(event).await
    }
}
