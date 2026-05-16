use std::sync::Arc;

use astrbot_core::{
    MessageEvent, ProviderContentPart, ProviderContextMessage, ProviderRequest, Result,
};
use async_trait::async_trait;

pub struct ProviderRequestEnvelope {
    pub request: ProviderRequest,
    pub explicit: bool,
}

impl ProviderRequestEnvelope {
    pub fn from_event(event: &MessageEvent) -> Option<Self> {
        if let Some(request) = event.provider_request() {
            return Some(Self {
                request: request.clone().with_event_defaults(event),
                explicit: true,
            });
        }

        let request = ProviderRequest::from_event(event);
        request.has_user_content().then_some(Self {
            request,
            explicit: false,
        })
    }
}

#[async_trait]
pub trait ProviderRequestDecorator: Send + Sync {
    async fn decorate(&self, event: &MessageEvent, request: &mut ProviderRequest) -> Result<()>;
}

pub struct NoopProviderRequestDecorator;

#[async_trait]
impl ProviderRequestDecorator for NoopProviderRequestDecorator {
    async fn decorate(&self, _event: &MessageEvent, _request: &mut ProviderRequest) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct CompositeProviderRequestDecorator {
    decorators: Vec<Arc<dyn ProviderRequestDecorator>>,
}

impl CompositeProviderRequestDecorator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_decorator(mut self, decorator: Arc<dyn ProviderRequestDecorator>) -> Self {
        self.decorators.push(decorator);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.decorators.is_empty()
    }
}

#[async_trait]
impl ProviderRequestDecorator for CompositeProviderRequestDecorator {
    async fn decorate(&self, event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        for decorator in &self.decorators {
            decorator.decorate(event, request).await?;
        }
        Ok(())
    }
}

#[async_trait]
pub trait AgentProviderPreferencePort: Send + Sync {
    async fn preferred_chat_provider_id(&self, event: &MessageEvent) -> Result<Option<String>>;
}

#[async_trait]
pub trait AgentSessionContextPort: Send + Sync {
    async fn context_messages(&self, event: &MessageEvent) -> Result<Vec<ProviderContextMessage>>;
}

#[async_trait]
pub trait AgentQuoteContextPort: Send + Sync {
    async fn quote_content_parts(&self, event: &MessageEvent) -> Result<Vec<ProviderContentPart>>;
}

pub struct ProviderPreferenceRequestDecorator {
    preference: Arc<dyn AgentProviderPreferencePort>,
}

impl ProviderPreferenceRequestDecorator {
    pub fn new(preference: Arc<dyn AgentProviderPreferencePort>) -> Self {
        Self { preference }
    }
}

#[async_trait]
impl ProviderRequestDecorator for ProviderPreferenceRequestDecorator {
    async fn decorate(&self, event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        if request
            .provider_id
            .as_deref()
            .is_some_and(|provider_id| !provider_id.trim().is_empty())
        {
            return Ok(());
        }

        request.provider_id = self
            .preference
            .preferred_chat_provider_id(event)
            .await?
            .map(|provider_id| provider_id.trim().to_string())
            .filter(|provider_id| !provider_id.is_empty());
        Ok(())
    }
}

pub struct SessionContextRequestDecorator {
    session_context: Arc<dyn AgentSessionContextPort>,
}

impl SessionContextRequestDecorator {
    pub fn new(session_context: Arc<dyn AgentSessionContextPort>) -> Self {
        Self { session_context }
    }
}

#[async_trait]
impl ProviderRequestDecorator for SessionContextRequestDecorator {
    async fn decorate(&self, event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        let mut contexts = self.session_context.context_messages(event).await?;
        if contexts.is_empty() {
            return Ok(());
        }

        contexts.append(&mut request.contexts);
        request.contexts = contexts;
        Ok(())
    }
}

pub struct QuoteContextRequestDecorator {
    quote_context: Arc<dyn AgentQuoteContextPort>,
}

impl QuoteContextRequestDecorator {
    pub fn new(quote_context: Arc<dyn AgentQuoteContextPort>) -> Self {
        Self { quote_context }
    }
}

#[async_trait]
impl ProviderRequestDecorator for QuoteContextRequestDecorator {
    async fn decorate(&self, event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        let mut quote_parts = self.quote_context.quote_content_parts(event).await?;
        if quote_parts.is_empty() {
            return Ok(());
        }

        quote_parts.append(&mut request.extra_user_content_parts);
        request.extra_user_content_parts = quote_parts;
        Ok(())
    }
}
