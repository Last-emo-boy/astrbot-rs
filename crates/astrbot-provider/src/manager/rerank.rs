use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use super::ProviderManager;
use crate::{RerankProvider, RerankRequest, RerankResponse};

#[async_trait]
impl RerankProvider for ProviderManager {
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse> {
        let provider = match request.provider_id.as_deref() {
            Some(provider_id) if !provider_id.trim().is_empty() => {
                self.rerank_provider(provider_id).ok_or_else(|| {
                    AstrbotError::Provider(format!(
                        "rerank provider {provider_id} is not configured"
                    ))
                })?
            }
            _ => self.default_rerank_provider().ok_or_else(|| {
                AstrbotError::Provider("no default rerank provider is configured".to_string())
            })?,
        };

        provider.rerank(request).await
    }

    async fn terminate(&self) -> Result<()> {
        ProviderManager::terminate(self).await
    }
}
