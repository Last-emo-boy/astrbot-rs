use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use super::ProviderManager;
use crate::{SpeechToTextProvider, SpeechToTextRequest, SpeechToTextResponse};

#[async_trait]
impl SpeechToTextProvider for ProviderManager {
    async fn transcribe(&self, request: SpeechToTextRequest) -> Result<SpeechToTextResponse> {
        let provider = match request.provider_id.as_deref() {
            Some(provider_id) if !provider_id.trim().is_empty() => {
                self.speech_to_text_provider(provider_id).ok_or_else(|| {
                    AstrbotError::Provider(format!(
                        "speech-to-text provider {provider_id} is not configured"
                    ))
                })?
            }
            _ => self.default_speech_to_text_provider().ok_or_else(|| {
                AstrbotError::Provider(
                    "no default speech-to-text provider is configured".to_string(),
                )
            })?,
        };

        provider.transcribe(request).await
    }

    async fn terminate(&self) -> Result<()> {
        ProviderManager::terminate(self).await
    }
}
