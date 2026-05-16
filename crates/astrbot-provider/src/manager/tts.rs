use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use super::ProviderManager;
use crate::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};

#[async_trait]
impl TextToSpeechProvider for ProviderManager {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        let provider = match request.provider_id.as_deref() {
            Some(provider_id) if !provider_id.trim().is_empty() => {
                self.text_to_speech_provider(provider_id).ok_or_else(|| {
                    AstrbotError::Provider(format!(
                        "text-to-speech provider {provider_id} is not configured"
                    ))
                })?
            }
            _ => self.default_text_to_speech_provider().ok_or_else(|| {
                AstrbotError::Provider(
                    "no default text-to-speech provider is configured".to_string(),
                )
            })?,
        };

        provider.synthesize(request).await
    }

    fn supports_streaming(&self) -> bool {
        self.supports_text_to_speech_streaming()
    }

    async fn terminate(&self) -> Result<()> {
        ProviderManager::terminate(self).await
    }
}
