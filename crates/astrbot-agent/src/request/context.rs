use std::sync::Arc;

use astrbot_core::{MessageEvent, ProviderRequest, Result};
use async_trait::async_trait;

use super::{
    AgentProviderPreferencePort, AgentQuoteContextPort, AgentSessionContextPort,
    ProviderRequestDecorator,
};

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
