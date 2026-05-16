use astrbot_core::Result;
use async_trait::async_trait;

use crate::chat::non_empty_option;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpeechToTextRequest {
    pub provider_id: Option<String>,
    pub audio_url: String,
}

impl SpeechToTextRequest {
    pub fn new(audio_url: impl Into<String>) -> Self {
        Self {
            provider_id: None,
            audio_url: audio_url.into(),
        }
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = non_empty_option(provider_id);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpeechToTextResponse {
    pub text: String,
}

impl SpeechToTextResponse {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

#[async_trait]
pub trait SpeechToTextProvider: Send + Sync {
    async fn transcribe(&self, request: SpeechToTextRequest) -> Result<SpeechToTextResponse>;

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MockSpeechToTextProvider {
    text: String,
}

impl MockSpeechToTextProvider {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

#[async_trait]
impl SpeechToTextProvider for MockSpeechToTextProvider {
    async fn transcribe(&self, request: SpeechToTextRequest) -> Result<SpeechToTextResponse> {
        if request.audio_url.trim().is_empty() {
            return Err(astrbot_core::AstrbotError::Provider(
                "speech-to-text request must contain an audio URL".to_string(),
            ));
        }

        Ok(SpeechToTextResponse::new(self.text.clone()))
    }
}
