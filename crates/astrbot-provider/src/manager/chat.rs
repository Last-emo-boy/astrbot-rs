use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use super::ProviderManager;
use crate::{ChatProvider, ChatRequest, ChatResponse};

#[async_trait]
impl ChatProvider for ProviderManager {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let provider = match request.provider_id.as_deref() {
            Some(provider_id) if !provider_id.trim().is_empty() => {
                self.chat_provider(provider_id).ok_or_else(|| {
                    AstrbotError::Provider(format!("chat provider {provider_id} is not configured"))
                })?
            }
            _ => self.default_chat_provider().ok_or_else(|| {
                AstrbotError::Provider("no default chat provider is configured".to_string())
            })?,
        };

        provider.chat(request).await
    }

    async fn terminate(&self) -> Result<()> {
        ProviderManager::terminate(self).await
    }
}
