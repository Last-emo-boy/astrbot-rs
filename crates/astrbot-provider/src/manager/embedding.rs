use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use super::ProviderManager;
use crate::{EmbeddingProvider, EmbeddingRequest, EmbeddingResponse};

#[async_trait]
impl EmbeddingProvider for ProviderManager {
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse> {
        let provider = match request.provider_id.as_deref() {
            Some(provider_id) if !provider_id.trim().is_empty() => {
                self.embedding_provider(provider_id).ok_or_else(|| {
                    AstrbotError::Provider(format!(
                        "embedding provider {provider_id} is not configured"
                    ))
                })?
            }
            _ => self.default_embedding_provider().ok_or_else(|| {
                AstrbotError::Provider("no default embedding provider is configured".to_string())
            })?,
        };

        provider.embed(request).await
    }

    fn dimensions(&self) -> Option<usize> {
        self.default_embedding_provider()
            .and_then(|provider| provider.dimensions())
    }

    async fn terminate(&self) -> Result<()> {
        ProviderManager::terminate(self).await
    }
}
